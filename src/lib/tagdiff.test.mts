// Tests for the control diff. Dependency-free: Node's built-in runner plus
// type stripping, so `npm test` needs nothing installed.
//
// The digest format is a contract with the tag-sharing website (tags/tagdiff.js
// produces the same structure from the same uesave JSON). These cover the parts
// that are easy to break in a port: which scalars count as settings, how
// bindings are bucketed per controller, and what the diff reports.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { extractDigest, diffDigests, diffTagRoot } from './tagdiff.ts';

/** Minimal parsed-save shape: uesave suffixes repeated names with _<index>. */
function makeRoot(overrides: Record<string, unknown> = {}) {
  return {
    save_game_type: '/Script/Rivals2.RivalsSaveGame',
    properties: {
      SavedPlayerTags_0: [
        {
          TagName_0: 'TESTER',
          ControlSettings_0: {
            bTapJumpEnabled_0: true,
            bDoubleTapDash_0: false,
            RightStickSetting_0: 'ERivalsRightStick::Attack',
            AirdodgeCardinalRoundingAngle_0: 12.34567,
            NotASetting_0: 'ignored',
            ActionMappings_0: {
              CustomActionMappings_0: [
                {
                  ActionName_0: 'Jump',
                  GamepadType_0: 'EGamepadType::Standard',
                  Key_0: { KeyName_0: 'SDL_GAMEPAD_BUTTON_NORTH' },
                  bShift_0: false,
                },
                {
                  ActionName_0: 'Attack',
                  bKeyboardKey_0: true,
                  Key_0: { KeyName_0: 'SpaceBar' },
                  bCtrl_0: true,
                },
              ],
            },
            ...(overrides as Record<string, never>),
          },
        },
      ],
    },
  };
}

test('extracts gameplay settings, ignoring non-setting scalars', () => {
  const d = extractDigest(makeRoot());
  assert.equal(d.settings.bTapJumpEnabled, true);
  assert.equal(d.settings.bDoubleTapDash, false);
  // enums are shortened past the "::"
  assert.equal(d.settings.RightStickSetting, 'Attack');
  // numeric settings are rounded to 4dp
  assert.equal(d.settings.AirdodgeCardinalRoundingAngle, 12.3457);
  // anything that isn't a known enum/number and doesn't start with "b" is out
  assert.ok(!('NotASetting' in d.settings));
});

test('buckets bindings by input type and records modifiers', () => {
  const d = extractDigest(makeRoot());
  assert.deepEqual(d.controllers.Standard.actions.Jump, ['SDL_GAMEPAD_BUTTON_NORTH']);
  // a keyboard key goes to its own bucket, with modifiers appended
  assert.deepEqual(d.controllers.Keyboard.actions.Attack, ['SpaceBar +Ctrl']);
});

test('diff reports only what differs, with friendly labels', () => {
  const digest = extractDigest(makeRoot());
  const baseline = {
    settings: {
      bTapJumpEnabled: false, // changed
      bDoubleTapDash: false, // same
      RightStickSetting: 'Strong', // changed
      AirdodgeCardinalRoundingAngle: 12.3457, // same
    },
    controllers: {
      Standard: { actions: { Jump: ['SDL_GAMEPAD_BUTTON_SOUTH'] }, axes: {} }, // changed
      Keyboard: { actions: { Attack: ['SpaceBar +Ctrl'] }, axes: {} }, // same
    },
  };
  const diff = diffDigests(digest, baseline);

  const settings = diff.groups.find((g) => g.scope === 'Gameplay settings');
  assert.ok(settings, 'a gameplay settings group is present');
  assert.equal(settings!.items.length, 2);
  assert.deepEqual(
    settings!.items.map((i) => i.label).sort(),
    ['Right stick', 'Tap Jump'],
  );
  // booleans render as On/Off, enums get their friendly name
  const rs = settings!.items.find((i) => i.label === 'Right stick')!;
  assert.equal(rs.from, 'Strong');
  assert.equal(rs.to, 'Attack');

  // the rebound gamepad button is reported with readable key names
  const pad = diff.groups.find((g) => g.scope.startsWith('Controller'));
  assert.ok(pad, 'a controller group is present');
  assert.deepEqual(pad!.items[0], {
    label: 'Jump',
    from: 'South (A)',
    to: 'North (Y)',
  });

  // Keyboard matched the baseline exactly, so it must not appear at all
  assert.ok(!diff.groups.some((g) => g.scope === 'Keyboard'));
  assert.equal(diff.count, 3);
});

test('an identical tag reports no changes', () => {
  const digest = extractDigest(makeRoot());
  assert.equal(diffDigests(digest, digest).count, 0);
});

test('diffTagRoot threads the baseline through', () => {
  const root = makeRoot();
  assert.equal(diffTagRoot(root, extractDigest(root)).count, 0);
});
