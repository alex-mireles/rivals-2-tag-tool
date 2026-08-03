# Cloud tag service

The service is a Node.js Lambda behind an API Gateway HTTP API. DynamoDB stores public tag metadata and expiring runtime state; S3 stores the private gzip-compressed files.

## First deployment

1. Install AWS SAM CLI and authenticate the AWS CLI for the target account.
2. From the repository root, run `pnpm install`.
3. Change into this directory and copy `samconfig.toml.example` to the ignored/local `samconfig.toml`:

   `cd infra`

   `Copy-Item samconfig.toml.example samconfig.toml`

   Supply the start.gg client ID, the Rivals of Aether II videogame ID, and the Route 53 public hosted-zone ID in `samconfig.toml`. The videogame ID can be resolved with the official start.gg `videogames` GraphQL query. Use `REPLACE_ME` for the client ID during the bootstrap deployment if the OAuth application does not exist yet.

   Route 53 creates a public hosted zone when the domain is registered. In the AWS console, open **Route 53 → Hosted zones → rivals2tagtool.com** and copy its **Hosted zone ID** (the value beginning with `Z`) into `Route53HostedZoneId`. Leave `ApiDomainName=api.rivals2tagtool.com` unless a different subdomain is desired.

4. Run the SAM commands from `infra`, where both `template.yaml` and `samconfig.toml` live:

   `sam validate --lint`

   `sam build`

   `sam deploy --guided --config-env production --profile <your-sso-profile>`

   After the first guided deployment, use `sam build` followed by `sam deploy --config-env production --profile <your-sso-profile>` for subsequent deployments.

   Running from this directory is important: SAM looks for `samconfig.toml` in the current directory, and `CodeUri: .` is relative to the SAM template.
   The deployment creates and DNS-validates an ACM certificate, maps the HTTP API to `api.rivals2tagtool.com`, and creates the Route 53 alias record. Certificate issuance and DNS propagation can add several minutes to the first deployment.
5. Copy the `OAuthRedirectUrl` stack output (`https://api.rivals2tagtool.com/v1/auth/callback`) into the start.gg OAuth application's redirect URI.
6. Replace the generated secret placeholder without putting credentials in CloudFormation or source control:

   `aws secretsmanager put-secret-value --secret-id <StartGgSecretArn> --secret-string '{"oauthClientSecret":"...","apiToken":"..."}'`

7. Redeploy with the final start.gg client ID if the initial deployment used a placeholder.
8. Set `VITE_CLOUD_API_BASE_URL` to the `ApiBaseUrl` output (`https://api.rivals2tagtool.com`) for desktop builds. The release workflow reads it from the matching GitHub Actions repository variable.

The AWS-generated `execute-api` URL remains available as the `AwsApiBaseUrl` output during migration and for troubleshooting. The app and start.gg OAuth configuration should use the custom-domain outputs.

### Running SAM from the repository root

If you prefer to remain in the repository root, provide both the template and config paths explicitly:

`sam validate --lint --template-file infra/template.yaml`

`sam build --template-file infra/template.yaml`

`sam deploy --template-file infra/.aws-sam/build/template.yaml --config-file infra/samconfig.toml --config-env production --profile <your-sso-profile>`

The Lambda discards OAuth access and refresh tokens after resolving the public start.gg identity. Expired DynamoDB runtime records are rejected by code even before DynamoDB's background TTL deletion occurs.

## Operator removal

Owners can delete their own tag in the app. For abuse handling, fetch the item by `startggUserId`, delete its referenced S3 object, and then delete the DynamoDB item. Do not delete an object based on user-provided paths.
