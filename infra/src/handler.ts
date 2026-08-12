import { randomBytes } from 'node:crypto';
import type { APIGatewayProxyEventV2, APIGatewayProxyResultV2 } from 'aws-lambda';
import { DynamoDBClient } from '@aws-sdk/client-dynamodb';
import { DeleteObjectCommand, GetObjectCommand, PutObjectCommand, S3Client } from '@aws-sdk/client-s3';
import { GetSecretValueCommand, SecretsManagerClient } from '@aws-sdk/client-secrets-manager';
import {
  BatchGetCommand,
  DeleteCommand,
  DynamoDBDocumentClient,
  GetCommand,
  PutCommand,
  QueryCommand,
  TransactWriteCommand,
  UpdateCommand,
} from '@aws-sdk/lib-dynamodb';
import { getSignedUrl } from '@aws-sdk/s3-request-presigner';
import {
  normalizeGamerTag,
  normalizeTournamentSlug,
  normalizeUserSlug,
  sha256,
  validateUpload,
  type UploadInput,
} from './domain.js';

const TAGS_TABLE = required('TAGS_TABLE');
const RUNTIME_TABLE = required('RUNTIME_TABLE');
const TAG_BUCKET = required('TAG_BUCKET');
const SECRET_ARN = required('SECRET_ARN');
const STARTGG_CLIENT_ID = required('STARTGG_CLIENT_ID');
const RIVALS2_VIDEOGAME_ID = required('RIVALS2_VIDEOGAME_ID');
const MAX_COMPRESSED_BYTES = Number(process.env.MAX_COMPRESSED_BYTES ?? 2 * 1024 * 1024);
const MAX_UNCOMPRESSED_BYTES = Number(process.env.MAX_UNCOMPRESSED_BYTES ?? 8 * 1024 * 1024);
const OAUTH_TTL_SECONDS = 10 * 60;
const SESSION_TTL_SECONDS = 12 * 60 * 60;
const CACHE_TTL_SECONDS = 10 * 60;
const STARTGG_GQL = 'https://api.start.gg/gql/alpha';

const ddb = DynamoDBDocumentClient.from(new DynamoDBClient({}), { marshallOptions: { removeUndefinedValues: true } });
const s3 = new S3Client({});
const secretsClient = new SecretsManagerClient({});
let secretCache: { oauthClientSecret: string; apiToken: string } | undefined;

interface CloudUser {
  startggUserId: string;
  slug: string;
  gamerTag: string;
}

interface TagItem {
  startggUserId: string;
  startggSlug: string;
  gamerTag: string;
  gamerTagKey: string;
  tagName: string;
  saveVersion?: number;
  compression: 'gzip';
  uncompressedSha256: string;
  compressedSize: number;
  uncompressedSize: number;
  objectKey: string;
  updatedAt: string;
}

function required(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`Missing environment variable ${name}`);
  return value;
}

function json(statusCode: number, body: unknown, headers: Record<string, string> = {}): APIGatewayProxyResultV2 {
  return {
    statusCode,
    headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store', ...headers },
    body: JSON.stringify(body),
  };
}

function html(statusCode: number, message: string): APIGatewayProxyResultV2 {
  return {
    statusCode,
    headers: { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' },
    body: `<!doctype html><html><head><meta charset="utf-8"><title>Rivals II Tag Tool</title></head><body style="font-family:system-ui;text-align:center;padding:4rem;background:#0e0c24;color:white"><h1>${message}</h1><p>You can close this window and return to the app.</p></body></html>`,
  };
}

function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

function randomToken(bytes = 32): string {
  return randomBytes(bytes).toString('base64url');
}

function parseBody<T>(event: APIGatewayProxyEventV2): T {
  if (!event.body) throw new HttpError(400, 'Missing request body');
  try {
    const body = event.isBase64Encoded ? Buffer.from(event.body, 'base64').toString('utf8') : event.body;
    return JSON.parse(body) as T;
  } catch {
    throw new HttpError(400, 'Invalid JSON body');
  }
}

class HttpError extends Error {
  constructor(public statusCode: number, message: string) {
    super(message);
  }
}

async function secrets(): Promise<{ oauthClientSecret: string; apiToken: string }> {
  if (secretCache) return secretCache;
  const result = await secretsClient.send(new GetSecretValueCommand({ SecretId: SECRET_ARN }));
  const parsed = JSON.parse(result.SecretString ?? '{}') as { oauthClientSecret?: string; apiToken?: string };
  if (!parsed.oauthClientSecret || !parsed.apiToken || parsed.oauthClientSecret === 'REPLACE_ME' || parsed.apiToken === 'REPLACE_ME') {
    throw new Error('The start.gg secret has not been configured');
  }
  secretCache = { oauthClientSecret: parsed.oauthClientSecret, apiToken: parsed.apiToken };
  return secretCache;
}

async function startgg<T>(query: string, variables: Record<string, unknown>, token: string): Promise<T> {
  const response = await fetch(STARTGG_GQL, {
    method: 'POST',
    headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
    body: JSON.stringify({ query, variables }),
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new HttpError(response.status === 429 ? 429 : 502, 'start.gg request failed');
  const payload = await response.json() as { data?: T; errors?: unknown[] };
  if (!payload.data || payload.errors?.length) throw new HttpError(502, 'start.gg returned an invalid response');
  return payload.data;
}

async function authenticated(event: APIGatewayProxyEventV2): Promise<CloudUser> {
  const header = event.headers.authorization ?? event.headers.Authorization;
  const match = header?.match(/^Bearer\s+(.+)$/i);
  if (!match) throw new HttpError(401, 'Authentication required');
  const item = await ddb.send(new GetCommand({ TableName: RUNTIME_TABLE, Key: { key: `SESSION#${sha256(match[1])}` } }));
  if (!item.Item || item.Item.expiresAt <= nowSeconds()) throw new HttpError(401, 'Session expired');
  return item.Item.user as CloudUser;
}

function publicTag(item: TagItem) {
  return {
    startggUserId: item.startggUserId,
    startggSlug: item.startggSlug,
    gamerTag: item.gamerTag,
    tagName: item.tagName,
    saveVersion: item.saveVersion ?? null,
    uncompressedSha256: item.uncompressedSha256,
    compressedSize: item.compressedSize,
    uncompressedSize: item.uncompressedSize,
    updatedAt: item.updatedAt,
  };
}

async function beginAuth(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const requestId = randomToken();
  const pollToken = randomToken();
  const expiresAt = nowSeconds() + OAUTH_TTL_SECONDS;
  const redirectUri = `https://${event.requestContext.domainName}/v1/auth/callback`;
  await ddb.send(new PutCommand({
    TableName: RUNTIME_TABLE,
    Item: { key: `AUTH#${requestId}`, pollHash: sha256(pollToken), redirectUri, status: 'pending', expiresAt },
    ConditionExpression: 'attribute_not_exists(#key)',
    ExpressionAttributeNames: { '#key': 'key' },
  }));
  const authorizationUrl = new URL('https://start.gg/oauth/authorize');
  authorizationUrl.searchParams.set('response_type', 'code');
  authorizationUrl.searchParams.set('client_id', STARTGG_CLIENT_ID);
  authorizationUrl.searchParams.set('scope', 'user.identity');
  authorizationUrl.searchParams.set('redirect_uri', redirectUri);
  authorizationUrl.searchParams.set('state', requestId);
  return json(201, { requestId, pollToken, authorizationUrl: authorizationUrl.toString(), expiresAt });
}

async function oauthCallback(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const code = event.queryStringParameters?.code;
  const state = event.queryStringParameters?.state;
  if (!code || !state) return html(400, 'Authentication failed');
  const key = `AUTH#${state}`;
  const auth = await ddb.send(new GetCommand({ TableName: RUNTIME_TABLE, Key: { key } }));
  if (!auth.Item || auth.Item.status !== 'pending' || auth.Item.expiresAt <= nowSeconds()) return html(400, 'Authentication request expired');
  const redirectUri = auth.Item.redirectUri as string | undefined;
  if (!redirectUri) return html(400, 'Authentication request is invalid');

  const secret = await secrets();
  const tokenResponse = await fetch('https://api.start.gg/oauth/access_token', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      grant_type: 'authorization_code', client_id: STARTGG_CLIENT_ID, client_secret: secret.oauthClientSecret,
      code, scope: 'user.identity', redirect_uri: redirectUri,
    }),
    signal: AbortSignal.timeout(15_000),
  });
  if (!tokenResponse.ok) return html(502, 'start.gg authentication failed');
  const token = await tokenResponse.json() as { access_token?: string };
  if (!token.access_token) return html(502, 'start.gg authentication failed');
  const identity = await startgg<{ currentUser: { id: string | number; slug: string; player: { gamerTag: string } | null } }>(
    'query CloudIdentity { currentUser { id slug player { gamerTag } } }', {}, token.access_token,
  );
  if (!identity.currentUser?.id || !identity.currentUser.slug) return html(502, 'start.gg identity unavailable');
  const user: CloudUser = {
    startggUserId: String(identity.currentUser.id),
    slug: identity.currentUser.slug,
    gamerTag: identity.currentUser.player?.gamerTag ?? identity.currentUser.slug,
  };
  await ddb.send(new UpdateCommand({
    TableName: RUNTIME_TABLE,
    Key: { key },
    UpdateExpression: 'SET #status = :completed, #user = :user',
    ConditionExpression: '#status = :pending AND expiresAt > :now',
    ExpressionAttributeNames: { '#status': 'status', '#user': 'user' },
    ExpressionAttributeValues: { ':completed': 'completed', ':pending': 'pending', ':user': user, ':now': nowSeconds() },
  }));
  await ddb.send(new UpdateCommand({
    TableName: TAGS_TABLE,
    Key: { startggUserId: user.startggUserId },
    UpdateExpression: 'SET startggSlug = :slug, gamerTag = :tag, gamerTagKey = :tagKey',
    ConditionExpression: 'attribute_exists(startggUserId)',
    ExpressionAttributeValues: { ':slug': user.slug, ':tag': user.gamerTag, ':tagKey': normalizeGamerTag(user.gamerTag) },
  })).catch((error: unknown) => {
    if ((error as { name?: string }).name !== 'ConditionalCheckFailedException') throw error;
  });
  return html(200, 'Authentication complete');
}

async function pollAuth(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const requestId = event.pathParameters?.id;
  const { pollToken } = parseBody<{ pollToken?: string }>(event);
  if (!requestId || !pollToken) throw new HttpError(400, 'Missing polling credentials');
  const key = `AUTH#${requestId}`;
  const result = await ddb.send(new GetCommand({ TableName: RUNTIME_TABLE, Key: { key } }));
  const item = result.Item;
  if (!item || item.expiresAt <= nowSeconds() || item.pollHash !== sha256(pollToken)) throw new HttpError(404, 'Authentication request expired');
  if (item.status === 'pending') return json(200, { status: 'pending' });
  if (item.status !== 'completed') throw new HttpError(409, 'Authentication request already consumed');

  const sessionToken = randomToken(48);
  const expiresAt = nowSeconds() + SESSION_TTL_SECONDS;
  await ddb.send(new TransactWriteCommand({ TransactItems: [
    { Update: {
      TableName: RUNTIME_TABLE, Key: { key }, UpdateExpression: 'SET #status = :consumed',
      ConditionExpression: '#status = :completed AND pollHash = :pollHash',
      ExpressionAttributeNames: { '#status': 'status' },
      ExpressionAttributeValues: { ':consumed': 'consumed', ':completed': 'completed', ':pollHash': sha256(pollToken) },
    } },
    { Put: {
      TableName: RUNTIME_TABLE,
      Item: { key: `SESSION#${sha256(sessionToken)}`, user: item.user, expiresAt },
      ConditionExpression: 'attribute_not_exists(#key)', ExpressionAttributeNames: { '#key': 'key' },
    } },
  ] }));
  return json(200, { status: 'complete', sessionToken, user: item.user, expiresAt });
}

async function endSession(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const header = event.headers.authorization ?? '';
  const token = header.replace(/^Bearer\s+/i, '');
  if (token) await ddb.send(new DeleteCommand({ TableName: RUNTIME_TABLE, Key: { key: `SESSION#${sha256(token)}` } }));
  return { statusCode: 204 };
}

async function searchTags(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const query = event.queryStringParameters?.query?.trim();
  if (!query || query.length > 200) throw new HttpError(400, 'A valid exact search is required');
  const slug = normalizeUserSlug(query);
  const result = await ddb.send(new QueryCommand({
    TableName: TAGS_TABLE,
    IndexName: slug ? 'SlugIndex' : 'GamerTagIndex',
    KeyConditionExpression: slug ? 'startggSlug = :value' : 'gamerTagKey = :value',
    ExpressionAttributeValues: { ':value': slug ?? normalizeGamerTag(query) },
    Limit: 25,
  }));
  return json(200, (result.Items as TagItem[] | undefined ?? []).map(publicTag));
}

async function downloadTag(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const userId = event.pathParameters?.userId;
  if (!userId) throw new HttpError(400, 'Missing user ID');
  const result = await ddb.send(new GetCommand({ TableName: TAGS_TABLE, Key: { startggUserId: userId } }));
  if (!result.Item) throw new HttpError(404, 'Cloud tag not found');
  const item = result.Item as TagItem;
  const location = await getSignedUrl(s3, new GetObjectCommand({ Bucket: TAG_BUCKET, Key: item.objectKey }), { expiresIn: 300 });
  return { statusCode: 302, headers: { location, 'cache-control': 'no-store' } };
}

async function uploadTag(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const user = await authenticated(event);
  const input = parseBody<UploadInput>(event);
  let validated: ReturnType<typeof validateUpload>;
  try {
    validated = validateUpload(input, MAX_COMPRESSED_BYTES, MAX_UNCOMPRESSED_BYTES);
  } catch (error) {
    throw new HttpError(400, (error as Error).message);
  }
  const updatedAt = new Date().toISOString();
  const objectKey = `tags/${user.startggUserId}/${input.uncompressedSha256.toLowerCase()}.r2tag.gz`;
  const prior = await ddb.send(new GetCommand({ TableName: TAGS_TABLE, Key: { startggUserId: user.startggUserId } }));
  await s3.send(new PutObjectCommand({
    Bucket: TAG_BUCKET, Key: objectKey, Body: validated.compressed,
    ContentType: 'application/vnd.rivals2.r2tag+gzip',
    Metadata: { sha256: input.uncompressedSha256.toLowerCase() },
    ServerSideEncryption: 'AES256',
  }));
  const item: TagItem = {
    startggUserId: user.startggUserId, startggSlug: user.slug, gamerTag: user.gamerTag,
    gamerTagKey: normalizeGamerTag(user.gamerTag), tagName: input.tagName,
    saveVersion: input.saveVersion ?? undefined, compression: 'gzip',
    uncompressedSha256: input.uncompressedSha256.toLowerCase(), compressedSize: validated.compressed.length,
    uncompressedSize: validated.uncompressedSize, objectKey, updatedAt,
  };
  await ddb.send(new PutCommand({ TableName: TAGS_TABLE, Item: item }));
  const oldKey = prior.Item?.objectKey as string | undefined;
  if (oldKey && oldKey !== objectKey) await s3.send(new DeleteObjectCommand({ Bucket: TAG_BUCKET, Key: oldKey })).catch(() => undefined);
  return json(200, publicTag(item));
}

async function deleteTag(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const user = await authenticated(event);
  const result = await ddb.send(new DeleteCommand({ TableName: TAGS_TABLE, Key: { startggUserId: user.startggUserId }, ReturnValues: 'ALL_OLD' }));
  if (result.Attributes?.objectKey) await s3.send(new DeleteObjectCommand({ Bucket: TAG_BUCKET, Key: result.Attributes.objectKey as string })).catch(() => undefined);
  return { statusCode: 204 };
}

async function cached<T>(key: string, loader: () => Promise<T>): Promise<T> {
  const cacheKey = `CACHE#${key}`;
  const hit = await ddb.send(new GetCommand({ TableName: RUNTIME_TABLE, Key: { key: cacheKey } }));
  if (hit.Item && hit.Item.expiresAt > nowSeconds()) return hit.Item.value as T;
  const value = await loader();
  await ddb.send(new PutCommand({ TableName: RUNTIME_TABLE, Item: { key: cacheKey, value, expiresAt: nowSeconds() + CACHE_TTL_SECONDS } }));
  return value;
}

async function tournamentTags(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const slug = normalizeTournamentSlug(event.queryStringParameters?.slug ?? '');
  const page = Math.max(1, Number.parseInt(event.queryStringParameters?.page ?? '1', 10));
  if (!slug || !Number.isSafeInteger(page) || page > 1000) throw new HttpError(400, 'Invalid tournament slug or page');
  const token = (await secrets()).apiToken;
  const events = await cached(`EVENTS#${slug}`, async () => startgg<{
    tournament: { id: string; name: string; events: Array<{ id: string; name: string }> } | null;
  }>('query CloudEvents($slug:String!,$game:[ID]!) { tournament(slug:$slug) { id name events(filter:{videogameId:$game}) { id name } } }',
  { slug, game: [RIVALS2_VIDEOGAME_ID] }, token));
  if (!events.tournament) throw new HttpError(404, 'Tournament not found');
  const eventIds = events.tournament.events.map((item) => item.id);
  if (eventIds.length === 0) return json(200, {
    tournamentName: events.tournament.name, tournamentSlug: slug, eventNames: [], page: 1,
    totalPages: 0, totalEntrants: 0, matches: [],
  });

  const participants = await cached(`PARTICIPANTS#${slug}#${page}`, async () => startgg<{
    tournament: { participants: { pageInfo: { total: number; totalPages: number }; nodes: Array<{ user: { id: string | number; slug: string } | null }> } };
  }>('query CloudParticipants($slug:String!,$eventIds:[ID],$page:Int!) { tournament(slug:$slug) { participants(query:{page:$page,perPage:50,filter:{eventIds:$eventIds}}) { pageInfo { total totalPages } nodes { user { id slug } } } } }',
  { slug, eventIds, page }, token));
  const ids = [...new Set(participants.tournament.participants.nodes.flatMap((node) => node.user ? [String(node.user.id)] : []))];
  // BatchGet returns UnprocessedKeys under throttling rather than failing.
  // Dropping them would silently hand a TO a bracket missing players' tags,
  // with nothing in the response to say anything was left out.
  const tagItems: TagItem[] = [];
  let pending = ids.map((startggUserId) => ({ startggUserId }));
  for (let attempt = 0; attempt < 4 && pending.length > 0; attempt += 1) {
    const batch = await ddb.send(new BatchGetCommand({ RequestItems: { [TAGS_TABLE]: { Keys: pending } } }));
    tagItems.push(...(batch.Responses?.[TAGS_TABLE] as TagItem[] | undefined ?? []));
    pending = (batch.UnprocessedKeys?.[TAGS_TABLE]?.Keys ?? []) as typeof pending;
  }
  if (pending.length > 0) throw new HttpError(503, 'The tag database is busy — try this page again');
  const matches = tagItems.map(publicTag);
  return json(200, {
    tournamentName: events.tournament.name,
    tournamentSlug: slug,
    eventNames: events.tournament.events.map((item) => item.name),
    page,
    totalPages: participants.tournament.participants.pageInfo.totalPages,
    totalEntrants: participants.tournament.participants.pageInfo.total,
    matches,
  });
}

export async function handler(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  const route = event.requestContext.routeKey;
  try {
    if (route === 'POST /v1/auth/requests') return await beginAuth(event);
    if (route === 'GET /v1/auth/callback') return await oauthCallback(event);
    if (route === 'POST /v1/auth/requests/{id}/poll') return await pollAuth(event);
    if (route === 'DELETE /v1/session') return await endSession(event);
    if (route === 'GET /v1/tags') return await searchTags(event);
    if (route === 'GET /v1/tags/{userId}/download') return await downloadTag(event);
    if (route === 'PUT /v1/me/tag') return await uploadTag(event);
    if (route === 'DELETE /v1/me/tag') return await deleteTag(event);
    if (route === 'GET /v1/tournaments/tags') return await tournamentTags(event);
    return json(404, { error: 'Route not found' });
  } catch (error) {
    if (error instanceof HttpError) return json(error.statusCode, { error: error.message });
    console.error(JSON.stringify({ level: 'error', route, error: error instanceof Error ? error.name : 'UnknownError' }));
    return json(500, { error: 'Internal server error' });
  }
}
