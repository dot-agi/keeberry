// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  AUTOCORRECT_OFF,
  AUTOCORRECT_ON,
  AUTOCORRECT_TOGGLE,
  AUTO_SHIFT_OFF,
  AUTO_SHIFT_ON,
  AUTO_SHIFT_TOGGLE,
  CATEGORY_LABELS,
  DEFAULT_LAYER_BASE,
  KEYCODES,
  autocorrect,
  BOOTLOADER,
  NONE,
  TRANSPARENT,
  ALT_REPEAT,
  CAPS_WORD,
  GRAVE_ESCAPE,
  LEADER,
  LAYER_LOCK,
  KEY_LOCK,
  REPEAT,
  SPACE_CADET_PAREN_LEFT,
  SPACE_CADET_PAREN_RIGHT,
  SPACE_CADET_ENTER,
  UNICODE_MAP_BASE,
  autoShift,
  classify,
  consumer,
  defaultLayer,
  encodeAction,
  fromUsage,
  gamepad,
  keycodeLabel,
  keycodeName,
  keycodeToken,
  keycodesByCategory,
  macro,
  momentary,
  mouse,
  oneShot,
  oneShotMod,
  tapdance,
  tapToggle,
  toLayer,
  toggle,
  unicodeMap,
  unicodeModeCycle,
  type KeyAction,
} from './keycode';

describe('classify (mirror of Keycode::classify)', () => {
  it('decodes the two engine sentinels', () => {
    expect(classify(0x0000)).toEqual({ kind: 'noop' });
    expect(classify(0x0001)).toEqual({ kind: 'transparent' });
  });

  it('decodes basic HID usages, with modifiers carved out of the middle', () => {
    expect(classify(0x0004)).toEqual({ kind: 'key', usage: 0x04 }); // A
    expect(classify(0x00df)).toEqual({ kind: 'key', usage: 0xdf }); // last low basic
    expect(classify(0x00e8)).toEqual({ kind: 'key', usage: 0xe8 }); // first high basic
    expect(classify(0x00ff)).toEqual({ kind: 'key', usage: 0xff });
  });

  it('decodes the eight modifiers by bit index (usage - 0xE0)', () => {
    expect(classify(0x00e0)).toEqual({ kind: 'modifier', index: 0 }); // LCtrl
    expect(classify(0x00e7)).toEqual({ kind: 'modifier', index: 7 }); // RGui
  });

  it('decodes MO(n) over the whole 0x5200..=0x521F region', () => {
    expect(classify(0x5200)).toEqual({ kind: 'momentary', layer: 0 });
    expect(classify(0x5201)).toEqual({ kind: 'momentary', layer: 1 });
    expect(classify(0x521f)).toEqual({ kind: 'momentary', layer: 31 });
  });

  it('decodes the TO/TG/TT/OSL layer-switch regions by target layer', () => {
    expect(classify(0x5300)).toEqual({ kind: 'to', layer: 0 });
    expect(classify(0x531f)).toEqual({ kind: 'to', layer: 31 });
    expect(classify(0x5400)).toEqual({ kind: 'tg', layer: 0 });
    expect(classify(0x541f)).toEqual({ kind: 'tg', layer: 31 });
    expect(classify(0x5800)).toEqual({ kind: 'tt', layer: 0 });
    expect(classify(0x581f)).toEqual({ kind: 'tt', layer: 31 });
    expect(classify(0x5900)).toEqual({ kind: 'osl', layer: 0 });
    expect(classify(0x591f)).toEqual({ kind: 'osl', layer: 31 });
  });

  it('decodes TD(n)/MACRO(n) across their regions and the lone BOOTLOADER', () => {
    expect(classify(0x5700)).toEqual({ kind: 'tapdance', index: 0 });
    expect(classify(0x57ff)).toEqual({ kind: 'tapdance', index: 255 });
    expect(classify(0x7700)).toEqual({ kind: 'macro', index: 0 });
    expect(classify(0x77ff)).toEqual({ kind: 'macro', index: 255 });
    expect(classify(0x7c00)).toEqual({ kind: 'boot' });
  });

  it('decodes the nine assigned mouse keys and treats the rest of the region as NoOp', () => {
    expect(classify(0x5500)).toEqual({ kind: 'mouse', code: 0 }); // move up
    expect(classify(0x5503)).toEqual({ kind: 'mouse', code: 3 }); // move right
    expect(classify(0x5508)).toEqual({ kind: 'mouse', code: 8 }); // wheel down
    // 0x5509..=0x55FF are reserved-but-unassigned, like the firmware's classify.
    expect(classify(0x5509)).toEqual({ kind: 'noop' });
    expect(classify(0x55ff)).toEqual({ kind: 'noop' });
  });

  it('decodes the 24 assigned gamepad keys and treats the rest of the region as NoOp', () => {
    expect(classify(0x5600)).toEqual({ kind: 'gamepad', code: 0 }); // button 0 (HID Button 1)
    expect(classify(0x560f)).toEqual({ kind: 'gamepad', code: 15 }); // button 15 (HID Button 16)
    expect(classify(0x5610)).toEqual({ kind: 'gamepad', code: 16 }); // axis X−
    expect(classify(0x5617)).toEqual({ kind: 'gamepad', code: 23 }); // axis Rz+
    // 0x5618..=0x56FF are reserved-but-unassigned, like the firmware's classify.
    expect(classify(0x5618)).toEqual({ kind: 'noop' });
    expect(classify(0x56ff)).toEqual({ kind: 'noop' });
  });

  it('decodes mod-tap / layer-tap, expanding the modifier selector', () => {
    expect(classify(0x2000)).toEqual({ kind: 'modtap', mods: 0x00, kc: 0x00 });
    expect(classify(0x2104)).toEqual({ kind: 'modtap', mods: 0x01, kc: 0x04 }); // LCtrl-tap-A
    expect(classify(0x3204)).toEqual({ kind: 'modtap', mods: 0x20, kc: 0x04 }); // RShift-tap-A
    expect(classify(0x3fff)).toEqual({ kind: 'modtap', mods: 0xf0, kc: 0xff }); // all right mods
    expect(classify(0x4000)).toEqual({ kind: 'layertap', layer: 0, kc: 0x00 });
    expect(classify(0x412c)).toEqual({ kind: 'layertap', layer: 1, kc: 0x2c }); // L1-tap-Space
    expect(classify(0x4fff)).toEqual({ kind: 'layertap', layer: 15, kc: 0xff });
  });

  it('decodes consumer usages from the top quarter (low 14 bits)', () => {
    expect(classify(0xc000)).toEqual({ kind: 'consumer', usage: 0x0000 });
    expect(classify(0xc0cd)).toEqual({ kind: 'consumer', usage: 0x00cd }); // Play/Pause
    expect(classify(0xffff)).toEqual({ kind: 'consumer', usage: 0x3fff });
  });

  it('decodes the nine behaviour-control codes plus OSM, treating the rest as NoOp', () => {
    expect(classify(0x5a00)).toEqual({ kind: 'layerlock' });
    expect(classify(0x5a01)).toEqual({ kind: 'autoshift', action: 'toggle' });
    expect(classify(0x5a02)).toEqual({ kind: 'autoshift', action: 'on' });
    expect(classify(0x5a03)).toEqual({ kind: 'autoshift', action: 'off' });
    expect(classify(0x5a04)).toEqual({ kind: 'leader' });
    expect(classify(0x5a05)).toEqual({ kind: 'capsword' });
    expect(classify(0x5a06)).toEqual({ kind: 'keylock' });
    expect(classify(0x5a07)).toEqual({ kind: 'repeat' });
    expect(classify(0x5a08)).toEqual({ kind: 'altrepeat' });
    // OSM: 0x5A09..=0x5A10, the HID modifier bit index in the low bits.
    expect(classify(0x5a09)).toEqual({ kind: 'oneshotmod', index: 0 }); // LCtrl
    expect(classify(0x5a10)).toEqual({ kind: 'oneshotmod', index: 7 }); // RGui
    // 0x5A11..=0x5A1F (a sibling wave's reservation) and the gaps above the assigned
    // Wave-5 sub-block stay reserved-but-unassigned, like the firmware's classify.
    expect(classify(0x5a11)).toEqual({ kind: 'noop' });
    expect(classify(0x5aff)).toEqual({ kind: 'noop' });
  });

  it('decodes the Wave-5 default-layer / grave-escape / Space-Cadet sub-block', () => {
    expect(classify(0x5a20)).toEqual({ kind: 'defaultlayer', layer: 0 });
    expect(classify(0x5a27)).toEqual({ kind: 'defaultlayer', layer: 7 });
    // 0x5A28 (layer 8) was the old top+1 — a NoOp at eight layers, now a valid DF code.
    expect(classify(0x5a28)).toEqual({ kind: 'defaultlayer', layer: 8 });
    // The 16-layer DF region fills 0x5A20..=0x5A2F, ending directly below GRAVE_ESCAPE.
    expect(classify(0x5a2f)).toEqual({ kind: 'defaultlayer', layer: 15 });
    expect(classify(0x5a30)).toEqual({ kind: 'gesc' });
    expect(classify(0x5a31)).toEqual({ kind: 'spacecadet', role: 'lspo' });
    expect(classify(0x5a32)).toEqual({ kind: 'spacecadet', role: 'rspc' });
    expect(classify(0x5a33)).toEqual({ kind: 'spacecadet', role: 'sent' });
    expect(classify(0x5a34)).toEqual({ kind: 'noop' });
  });

  it('decodes the autocorrect control sub-block (0x5A40..=0x5A42)', () => {
    expect(classify(0x5a40)).toEqual({ kind: 'autocorrect', action: 'toggle' });
    expect(classify(0x5a41)).toEqual({ kind: 'autocorrect', action: 'on' });
    expect(classify(0x5a42)).toEqual({ kind: 'autocorrect', action: 'off' });
    // 0x5A3F (below) and 0x5A43 (above) bound the block; both fall through to NoOp.
    expect(classify(0x5a3f)).toEqual({ kind: 'noop' });
    expect(classify(0x5a43)).toEqual({ kind: 'noop' });
  });

  it('decodes the Unicode mode-cycle / UM-map sub-block', () => {
    // 0x5A50 leads the block; 0x5A51..=0x5A60 is the 16-slot UM map; 0x5A61 is the gap above it.
    expect(classify(0x5a50)).toEqual({ kind: 'unicodecycle' });
    expect(classify(0x5a51)).toEqual({ kind: 'unicodemap', index: 0 });
    expect(classify(0x5a60)).toEqual({ kind: 'unicodemap', index: 15 });
    expect(classify(0x5a61)).toEqual({ kind: 'noop' });
  });

  it('decodes every unassigned encoding as a safe NoOp', () => {
    // 0x5220/0x5320/0x5A11 are the gaps just above MO/TO and the behaviour codes;
    // each region is bounded.
    for (const raw of [0x0002, 0x0003, 0x0100, 0x51ff, 0x5220, 0x5320, 0x5a11, 0xbfff]) {
      expect(classify(raw)).toEqual({ kind: 'noop' });
    }
  });
});

describe('encode ∘ decode round-trips', () => {
  // Representative canonical encodings spanning every region and its boundaries.
  const canonical = [
    0x0000, 0x0001, 0x0004, 0x00df, 0x00e0, 0x00e7, 0x00e8, 0x00ff, 0x2000, 0x2104, 0x3204, 0x3fff,
    0x4000, 0x412c, 0x4fff, 0x5200, 0x5201, 0x521f, 0x5300, 0x531f, 0x5400, 0x541f, 0x5500, 0x5508,
    0x5600, 0x560f, 0x5610, 0x5617, 0x5700, 0x57ff, 0x5800, 0x581f, 0x5900, 0x591f, 0x5a00, 0x5a01,
    0x5a02, 0x5a03, 0x5a04, 0x5a05, 0x5a06, 0x5a07, 0x5a08, 0x5a09, 0x5a10, 0x5a20, 0x5a27, 0x5a30,
    0x5a31, 0x5a32, 0x5a33, 0x5a40, 0x5a41, 0x5a42, 0x5a50, 0x5a51, 0x5a60, 0x7700, 0x77ff, 0x7c00,
    0xc000, 0xc0cd, 0xffff,
  ];

  it('encodeAction(classify(raw)) === raw for every canonical code', () => {
    for (const raw of canonical) {
      expect(encodeAction(classify(raw))).toBe(raw);
    }
  });

  it('classify(encodeAction(action)) deep-equals the action', () => {
    const actions: KeyAction[] = [
      { kind: 'noop' },
      { kind: 'transparent' },
      { kind: 'key', usage: 0x16 },
      { kind: 'modifier', index: 3 },
      { kind: 'momentary', layer: 1 },
      { kind: 'to', layer: 2 },
      { kind: 'tg', layer: 3 },
      { kind: 'tt', layer: 1 },
      { kind: 'osl', layer: 2 },
      { kind: 'modtap', mods: 0x01, kc: 0x16 },
      { kind: 'modtap', mods: 0x20, kc: 0x04 },
      { kind: 'layertap', layer: 2, kc: 0x2c },
      { kind: 'tapdance', index: 0 },
      { kind: 'macro', index: 2 },
      { kind: 'mouse', code: 0 },
      { kind: 'mouse', code: 8 },
      { kind: 'gamepad', code: 0 },
      { kind: 'gamepad', code: 15 },
      { kind: 'gamepad', code: 23 },
      { kind: 'boot' },
      { kind: 'layerlock' },
      { kind: 'autoshift', action: 'toggle' },
      { kind: 'autoshift', action: 'on' },
      { kind: 'autoshift', action: 'off' },
      { kind: 'leader' },
      { kind: 'capsword' },
      { kind: 'keylock' },
      { kind: 'repeat' },
      { kind: 'altrepeat' },
      { kind: 'oneshotmod', index: 0 },
      { kind: 'oneshotmod', index: 7 },
      { kind: 'defaultlayer', layer: 0 },
      { kind: 'defaultlayer', layer: 7 },
      { kind: 'gesc' },
      { kind: 'spacecadet', role: 'lspo' },
      { kind: 'spacecadet', role: 'rspc' },
      { kind: 'spacecadet', role: 'sent' },
      { kind: 'autocorrect', action: 'toggle' },
      { kind: 'autocorrect', action: 'on' },
      { kind: 'autocorrect', action: 'off' },
      { kind: 'unicodecycle' },
      { kind: 'unicodemap', index: 0 },
      { kind: 'unicodemap', index: 15 },
      { kind: 'consumer', usage: 0xe9 },
    ];
    for (const action of actions) {
      expect(classify(encodeAction(action))).toEqual(action);
    }
  });

  it('round-trips every named catalogue keycode (all are canonical)', () => {
    for (const kc of KEYCODES) {
      expect(encodeAction(classify(kc.raw))).toBe(kc.raw);
    }
  });
});

describe('constructors (mirror of from_usage / momentary / consumer)', () => {
  it('fromUsage casts a u8 usage straight into the u16 space', () => {
    expect(fromUsage(0x04)).toBe(0x0004);
    expect(fromUsage(0xe3)).toBe(0x00e3);
  });

  it('momentary sets MO_BASE and masks the layer to five bits', () => {
    expect(momentary(0)).toBe(0x5200);
    expect(momentary(1)).toBe(0x5201);
    expect(momentary(31)).toBe(0x521f);
    expect(momentary(32)).toBe(0x5200); // 32 & 0x1F === 0
  });

  it('consumer sets CONSUMER_BASE and masks the usage to 14 bits', () => {
    expect(consumer(0xcd)).toBe(0xc0cd);
    expect(consumer(0x223)).toBe(0xc223);
    expect(consumer(0x4000)).toBe(0xc000); // 0x4000 & 0x3FFF === 0
  });
});

describe('catalogue and labels', () => {
  it('exposes the standard A and modifier usages with exact labels', () => {
    expect(keycodeLabel(fromUsage(0x04))).toBe('A');
    expect(keycodeName(fromUsage(0xe1))).toBe('Left Shift');
    expect(keycodeLabel(NONE)).toBe('None');
    expect(keycodeLabel(TRANSPARENT)).toBe('Trans');
  });

  it('derives labels for codes that are not in the catalogue', () => {
    expect(keycodeLabel(momentary(2))).toBe('MomentaryLayer(2)');
    expect(keycodeLabel(toLayer(1))).toBe('ActivateLayer(1)');
    expect(keycodeLabel(toggle(2))).toBe('ToggleLayer(2)');
    expect(keycodeLabel(tapToggle(3))).toBe('TapToggleLayer(3)');
    expect(keycodeLabel(oneShot(0))).toBe('OneShotLayer(0)');
    expect(keycodeLabel(tapdance(3))).toBe('TapDance(3)');
    expect(keycodeLabel(macro(1))).toBe('Macro(1)');
    expect(keycodeLabel(BOOTLOADER)).toBe('Bootloader');
    expect(keycodeLabel(consumer(0x100))).toBe('CC 0x100');
    expect(keycodeLabel(0x0050)).toBe('←'); // catalogue Left arrow
    expect(keycodeLabel(0x00aa)).toBe('0xAA'); // unlisted basic usage
  });

  it('labels and names the behaviour-control codes (no catalogue entry, like Boot)', () => {
    expect(keycodeLabel(LAYER_LOCK)).toBe('LayerLock');
    expect(keycodeName(LAYER_LOCK)).toBe('Layer Lock');
    expect(keycodeLabel(AUTO_SHIFT_TOGGLE)).toBe('AutoShiftToggle');
    expect(keycodeName(AUTO_SHIFT_ON)).toBe('Auto-Shift On');
    expect(keycodeName(AUTO_SHIFT_OFF)).toBe('Auto-Shift Off');
    expect(keycodeLabel(LEADER)).toBe('Leader');
    expect(keycodeName(LEADER)).toBe('Leader Key');
    expect(keycodeLabel(CAPS_WORD)).toBe('CapsWord');
    expect(keycodeName(CAPS_WORD)).toBe('Caps Word');
    expect(keycodeLabel(KEY_LOCK)).toBe('KeyLock');
    expect(keycodeName(KEY_LOCK)).toBe('Key Lock');
    expect(keycodeLabel(REPEAT)).toBe('Repeat');
    expect(keycodeName(REPEAT)).toBe('Repeat Key');
    expect(keycodeLabel(ALT_REPEAT)).toBe('AltRepeat');
    expect(keycodeName(ALT_REPEAT)).toBe('Alternate Repeat Key');
    // OSM names its modifier; the constructor mirrors the firmware encoding.
    expect(keycodeLabel(oneShotMod(1))).toBe('OneShotMod(ShiftLeft)');
    expect(keycodeName(oneShotMod(1))).toBe('One-Shot Left Shift');
    expect(oneShotMod(0)).toBe(0x5a09);
    expect(oneShotMod(7)).toBe(0x5a10);
    // The auto-shift constructor mirrors the firmware encoding.
    expect(autoShift('toggle')).toBe(AUTO_SHIFT_TOGGLE);
    expect(autoShift('on')).toBe(AUTO_SHIFT_ON);
    expect(autoShift('off')).toBe(AUTO_SHIFT_OFF);
  });

  it('labels and names the Wave-5 default-layer / grave-escape / Space-Cadet codes', () => {
    expect(keycodeLabel(defaultLayer(3))).toBe('DefaultLayer(3)');
    expect(keycodeName(defaultLayer(3))).toBe('Default Layer 3');
    expect(keycodeLabel(GRAVE_ESCAPE)).toBe('GraveEscape');
    expect(keycodeName(GRAVE_ESCAPE)).toBe('Grave-Escape (Esc / ` / ~)');
    expect(keycodeLabel(SPACE_CADET_PAREN_LEFT)).toBe('SpaceCadetParenLeft');
    expect(keycodeName(SPACE_CADET_PAREN_RIGHT)).toBe('Space-Cadet: Right Shift / )');
    expect(keycodeName(SPACE_CADET_ENTER)).toBe('Space-Cadet: Right Shift / Enter');
    // The default-layer constructor mirrors the firmware encoding and masks the layer.
    expect(defaultLayer(0)).toBe(DEFAULT_LAYER_BASE);
    expect(defaultLayer(15)).toBe(DEFAULT_LAYER_BASE + 15);
    expect(defaultLayer(16)).toBe(DEFAULT_LAYER_BASE); // 16 % 16 === 0
  });

  it('labels and names the autocorrect control codes (no catalogue entry, like Boot)', () => {
    expect(keycodeLabel(AUTOCORRECT_TOGGLE)).toBe('AutocorrectToggle');
    expect(keycodeName(AUTOCORRECT_TOGGLE)).toBe('Autocorrect Toggle');
    expect(keycodeLabel(AUTOCORRECT_ON)).toBe('AutocorrectOn');
    expect(keycodeName(AUTOCORRECT_ON)).toBe('Autocorrect On');
    expect(keycodeLabel(AUTOCORRECT_OFF)).toBe('AutocorrectOff');
    expect(keycodeName(AUTOCORRECT_OFF)).toBe('Autocorrect Off');
    // The autocorrect constructor mirrors the firmware encoding.
    expect(autocorrect('toggle')).toBe(AUTOCORRECT_TOGGLE);
    expect(autocorrect('on')).toBe(AUTOCORRECT_ON);
    expect(autocorrect('off')).toBe(AUTOCORRECT_OFF);
  });

  it('labels and names the Unicode mode-cycle / UM-map codes', () => {
    expect(keycodeLabel(unicodeModeCycle())).toBe('UnicodeModeCycle');
    expect(keycodeName(unicodeModeCycle())).toBe('Unicode: cycle OS input mode');
    expect(keycodeLabel(unicodeMap(0))).toBe('UnicodeMap(0)');
    expect(keycodeLabel(unicodeMap(15))).toBe('UnicodeMap(15)');
    expect(keycodeName(unicodeMap(7))).toBe('Unicode Map 7');
    // The UM constructor masks the slot into the 16-entry map (`Keycode::unicode_map`).
    expect(unicodeMap(0)).toBe(UNICODE_MAP_BASE);
    expect(unicodeMap(16)).toBe(UNICODE_MAP_BASE); // 16 % 16 === 0
  });

  it('exposes the mouse keys from the catalogue with exact labels', () => {
    expect(keycodeLabel(mouse(4))).toBe('LMB'); // button 1
    expect(keycodeName(mouse(0))).toBe('Mouse Up');
    expect(keycodeName(mouse(8))).toBe('Wheel Down');
  });

  it('exposes the gamepad keys from the catalogue with exact labels', () => {
    expect(keycodeLabel(gamepad(0))).toBe('Pad 1'); // button 0 is HID Button 1
    expect(keycodeName(gamepad(15))).toBe('Gamepad Button 16');
    expect(keycodeLabel(gamepad(16))).toBe('X -'); // first axis, negative end
    expect(keycodeName(gamepad(23))).toBe('Joystick Rz+');
  });

  it('groups keycodes under their categories and labels every category', () => {
    expect(keycodesByCategory('letters')).toHaveLength(26);
    expect(keycodesByCategory('numbers')).toHaveLength(10);
    expect(keycodesByCategory('function')).toHaveLength(12);
    expect(keycodesByCategory('modifiers')).toHaveLength(8);
    expect(keycodesByCategory('mouse')).toHaveLength(9);
    expect(keycodesByCategory('gamepad')).toHaveLength(24);
    // Every category used by a keycode has a display label.
    const labelled = new Set(CATEGORY_LABELS.map((c) => c.category));
    for (const kc of KEYCODES) {
      expect(labelled.has(kc.category)).toBe(true);
    }
  });

  it('gives every catalogue keycode a non-empty W3C token', () => {
    for (const kc of KEYCODES) {
      expect(kc.token.length).toBeGreaterThan(0);
    }
  });
});

describe('keycodeToken (canonical W3C / KKN token)', () => {
  it('returns the catalogue token for listed keys', () => {
    expect(keycodeToken(fromUsage(0x04))).toBe('KeyA');
    expect(keycodeToken(fromUsage(0x52))).toBe('ArrowUp');
    expect(keycodeToken(fromUsage(0xe0))).toBe('ControlLeft');
    expect(keycodeToken(consumer(0xe9))).toBe('AudioVolumeUp');
    expect(keycodeToken(NONE)).toBe('None');
  });

  it('derives function-style tokens for layer / tap-hold / behaviour codes', () => {
    expect(keycodeToken(momentary(2))).toBe('MomentaryLayer(2)');
    // 0x2104 = mod-tap hold Left Control, tap A; 0x412C = layer-tap hold layer 1, tap Space.
    expect(keycodeToken(0x2104)).toBe('ModTap(ControlLeft, KeyA)');
    expect(keycodeToken(0x412c)).toBe('LayerTap(1, Space)');
    expect(keycodeToken(oneShotMod(1))).toBe('OneShotMod(ShiftLeft)');
    expect(keycodeToken(LAYER_LOCK)).toBe('LayerLock');
    expect(keycodeToken(SPACE_CADET_ENTER)).toBe('SpaceCadetEnter');
  });
});
