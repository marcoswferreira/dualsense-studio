import { DualTriggerConfig, LightingConfig, TriggerEffect } from '../types/models';

export interface LiveGamepadState {
  connected: boolean;
  id: string;
  leftStick: { x: number; y: number };
  rightStick: { x: number; y: number };
  l2: number;
  r2: number;
  buttons: {
    cross: boolean;
    circle: boolean;
    square: boolean;
    triangle: boolean;
    l1: boolean;
    r1: boolean;
    l3: boolean;
    r3: boolean;
    create: boolean;
    options: boolean;
    ps: boolean;
    touchpad: boolean;
    dpadUp: boolean;
    dpadDown: boolean;
    dpadLeft: boolean;
    dpadRight: boolean;
  };
}

interface HidDeviceLike {
  opened: boolean;
  productName?: string;
  open(): Promise<void>;
  close(): Promise<void>;
  sendReport(reportId: number, data: BufferSource): Promise<void>;
}

interface NavigatorWithHid extends Navigator {
  hid?: {
    requestDevice(options: {
      filters: Array<{ vendorId: number; productId?: number }>;
    }): Promise<HidDeviceLike[]>;
  };
}

const SONY_VENDOR_ID = 0x054c;
const DUALSENSE_PID = 0x0ce6;
const DUALSENSE_EDGE_PID = 0x0df2;

export interface BackendDualSenseInput {
  connection: 'usb' | 'bluetooth';
  left_stick_x: number;
  left_stick_y: number;
  right_stick_x: number;
  right_stick_y: number;
  l2_trigger: number;
  r2_trigger: number;
  cross: boolean;
  circle: boolean;
  square: boolean;
  triangle: boolean;
  dpad_up: boolean;
  dpad_down: boolean;
  dpad_left: boolean;
  dpad_right: boolean;
  l1: boolean;
  r1: boolean;
  l2_click: boolean;
  r2_click: boolean;
  l3: boolean;
  r3: boolean;
  create: boolean;
  options: boolean;
  ps_button: boolean;
  touchpad_click: boolean;
  mic_mute: boolean;
  fn_left?: boolean;
  fn_right?: boolean;
  paddle_left?: boolean;
  paddle_right?: boolean;
  motion?: {
    gyro_x: number;
    gyro_y: number;
    gyro_z: number;
    accel_x: number;
    accel_y: number;
    accel_z: number;
  };
  touch1?: {
    active: boolean;
    id: number;
    x: number;
    y: number;
  };
}

export interface BackendInputResponse {
  connected: boolean;
  name: string;
  input: BackendDualSenseInput;
}

export function backendInputToLiveGamepad(resp: BackendInputResponse): LiveGamepadState {
  const inp = resp.input;
  const normAxis = (v: number) => Math.max(-1, Math.min(1, (v - 128) / 128));
  return {
    connected: resp.connected,
    id: `${resp.name} (${inp.connection.toUpperCase()} HID)`,
    leftStick: {
      x: normAxis(inp.left_stick_x),
      y: normAxis(inp.left_stick_y),
    },
    rightStick: {
      x: normAxis(inp.right_stick_x),
      y: normAxis(inp.right_stick_y),
    },
    l2: Math.round((inp.l2_trigger / 255) * 100),
    r2: Math.round((inp.r2_trigger / 255) * 100),
    buttons: {
      cross: inp.cross,
      circle: inp.circle,
      square: inp.square,
      triangle: inp.triangle,
      l1: inp.l1,
      r1: inp.r1,
      l3: inp.l3,
      r3: inp.r3,
      create: inp.create,
      options: inp.options,
      ps: inp.ps_button,
      touchpad: inp.touchpad_click,
      dpadUp: inp.dpad_up,
      dpadDown: inp.dpad_down,
      dpadLeft: inp.dpad_left,
      dpadRight: inp.dpad_right,
    },
  };
}

function writeU16LE(bytes: Uint8Array, offset: number, val: number): void {
  bytes[offset] = val & 0xff;
  bytes[offset + 1] = (val >>> 8) & 0xff;
}

function writeU32LE(bytes: Uint8Array, offset: number, val: number): void {
  bytes[offset] = val & 0xff;
  bytes[offset + 1] = (val >>> 8) & 0xff;
  bytes[offset + 2] = (val >>> 16) & 0xff;
  bytes[offset + 3] = (val >>> 24) & 0xff;
}

function compileTriggerPayload(effect: TriggerEffect): Uint8Array {
  const bytes = new Uint8Array(11);
  switch (effect.mode) {
    case 'off':
      bytes[0] = 0x05;
      break;
    case 'feedback': {
      if (effect.force <= 0) {
        bytes[0] = 0x05;
        break;
      }
      const start = Math.max(0, Math.min(9, effect.start_position));
      const f = (Math.max(1, Math.min(8, effect.force)) - 1) & 0x07;
      let activeZones = 0;
      let forceZones = 0;
      for (let i = start; i < 10; i++) {
        activeZones |= 1 << i;
        forceZones = (forceZones | (f << (3 * i))) >>> 0;
      }
      bytes[0] = 0x21;
      writeU16LE(bytes, 1, activeZones);
      writeU32LE(bytes, 3, forceZones);
      break;
    }
    case 'weapon': {
      if (effect.force <= 0) {
        bytes[0] = 0x05;
        break;
      }
      const start = Math.max(0, Math.min(8, effect.start_position));
      const end = Math.max(start + 1, Math.min(9, effect.end_position));
      const startAndStop = (1 << start) | (1 << end);
      const f = (Math.max(1, Math.min(8, effect.force)) - 1) & 0x07;
      bytes[0] = 0x25;
      writeU16LE(bytes, 1, startAndStop);
      bytes[3] = f;
      break;
    }
    case 'bow': {
      const start = Math.max(0, Math.min(8, effect.start_position));
      const end = Math.max(start + 1, Math.min(9, effect.end_position));
      const startAndStop = (1 << start) | (1 << end);
      const f = (Math.max(1, Math.min(8, effect.force)) - 1) & 0x07;
      const sf = (Math.max(1, Math.min(8, effect.snap_force)) - 1) & 0x07;
      bytes[0] = 0x22;
      writeU16LE(bytes, 1, startAndStop);
      bytes[3] = (sf << 3) | f;
      break;
    }
    case 'galloping': {
      const start = Math.max(0, Math.min(8, effect.start_position));
      const end = Math.max(start + 1, Math.min(9, effect.end_position));
      const startAndStop = (1 << start) | (1 << end);
      const ff = Math.max(0, Math.min(6, effect.first_foot));
      const sf = Math.max(ff + 1, Math.min(7, effect.second_foot));
      bytes[0] = 0x23;
      writeU16LE(bytes, 1, startAndStop);
      bytes[3] = (sf << 3) | ff;
      bytes[4] = Math.max(1, effect.frequency & 0xff);
      break;
    }
    case 'machine_gun': {
      if (effect.strength <= 0 || effect.frequency <= 0) {
        bytes[0] = 0x05;
        break;
      }
      const start = Math.max(0, Math.min(9, effect.start_position));
      const end = Math.max(start, Math.min(9, effect.end_position));
      const amp = (Math.max(1, Math.min(8, effect.strength)) - 1) & 0x07;
      let activeZones = 0;
      let ampZones = 0;
      for (let i = start; i <= end; i++) {
        activeZones |= 1 << i;
        ampZones = (ampZones | (amp << (3 * i))) >>> 0;
      }
      bytes[0] = 0x26;
      writeU16LE(bytes, 1, activeZones);
      writeU32LE(bytes, 3, ampZones);
      bytes[9] = effect.frequency & 0xff;
      break;
    }
    case 'machine': {
      const start = Math.max(0, Math.min(8, effect.start_position));
      const end = Math.max(start + 1, Math.min(9, effect.end_position));
      const startAndStop = (1 << start) | (1 << end);
      const sa = Math.max(0, Math.min(7, effect.strength_a));
      const sb = Math.max(0, Math.min(7, effect.strength_b));
      bytes[0] = 0x27;
      writeU16LE(bytes, 1, startAndStop);
      bytes[3] = (sb << 3) | sa;
      bytes[4] = Math.max(1, effect.frequency & 0xff);
      bytes[5] = effect.period & 0xff;
      break;
    }
    case 'vibration': {
      if (effect.amplitude <= 0 || effect.frequency <= 0) {
        bytes[0] = 0x05;
        break;
      }
      const pos = Math.max(0, Math.min(9, effect.position));
      const amp =
        effect.amplitude <= 8
          ? (effect.amplitude - 1) & 0x07
          : Math.floor((Math.min(63, effect.amplitude) * 7) / 63) & 0x07;
      let activeZones = 0;
      let ampZones = 0;
      for (let i = pos; i < 10; i++) {
        activeZones |= 1 << i;
        ampZones = (ampZones | (amp << (3 * i))) >>> 0;
      }
      bytes[0] = 0x26;
      writeU16LE(bytes, 1, activeZones);
      writeU32LE(bytes, 3, ampZones);
      bytes[9] = effect.frequency & 0xff;
      break;
    }
    case 'game_cube':
      bytes.set([0x02, 0x90, 0xa0, 0xff], 0);
      break;
    case 'choppy':
      bytes.set([0x02, 0x02, 0x27, 0x03, 0x27, 0x26, 0x02], 0);
      break;
    case 'very_soft':
      bytes.set([0x01, 0x80, 0x20], 0);
      break;
    case 'soft':
      bytes.set([0x01, 0x45, 0xa0], 0);
      break;
    case 'medium':
      bytes.set([0x02, 0x02, 0x01, 0x02, 0x02, 0x01, 0x22, 0xff], 0);
      break;
    case 'hard':
      bytes.set([0x01, 0x20, 0xff], 0);
      break;
    case 'very_hard':
      bytes.set([0x01, 0x10, 0xff], 0);
      break;
    case 'hardest':
      bytes.set([0x01, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff], 0);
      break;
    case 'rigid':
      bytes.set([0x01, 0x00, 0xff], 0);
      break;
    case 'vibrate_trigger_pulse':
      bytes.set([0x02, 0x25, 0x06, 0x02, 0x27, 0x25, 0x23], 0);
      break;
    case 'custom_zone': {
      let activeZones = 0;
      let forceZones = 0;
      for (let i = 0; i < 10; i++) {
        const z = effect.zones[i] ?? 0;
        if (z > 0) {
          const level = z <= 8 ? (z - 1) & 0x07 : Math.floor((Math.min(255, z) * 7) / 255) & 0x07;
          activeZones |= 1 << i;
          forceZones = (forceZones | (level << (3 * i))) >>> 0;
        }
      }
      if (activeZones === 0) {
        bytes[0] = 0x05;
      } else {
        bytes[0] = 0x21;
        writeU16LE(bytes, 1, activeZones);
        writeU32LE(bytes, 3, forceZones);
      }
      break;
    }
  }
  return bytes;
}

function playerLedBitmask(led: LightingConfig['player_led']): number {
  switch (led) {
    case 'off':
      return 0x00;
    case 'player1':
      return 0x04;
    case 'player2':
      return 0x0a;
    case 'player3':
      return 0x15;
    case 'player4':
      return 0x1b;
    case 'all':
      return 0x1f;
  }
}

function micLedByte(led: LightingConfig['mic_led']): number {
  switch (led) {
    case 'off':
      return 0x00;
    case 'solid':
      return 0x01;
    case 'pulse':
      return 0x02;
  }
}

export class WebHidDualSenseController {
  private device: HidDeviceLike | null = null;

  static isSupported(): boolean {
    return typeof navigator !== 'undefined' && 'hid' in navigator;
  }

  isConnected(): boolean {
    return Boolean(this.device && this.device.opened);
  }

  getDeviceName(): string {
    return this.device?.productName || 'Sony DualSense Wireless Controller';
  }

  async connect(): Promise<boolean> {
    const nav = navigator as NavigatorWithHid;
    if (!nav.hid) return false;
    try {
      const devices = await nav.hid.requestDevice({
        filters: [
          { vendorId: SONY_VENDOR_ID, productId: DUALSENSE_PID },
          { vendorId: SONY_VENDOR_ID, productId: DUALSENSE_EDGE_PID },
        ],
      });
      if (!devices.length) return false;
      const dev = devices[0];
      if (!dev.opened) {
        await dev.open();
      }
      this.device = dev;
      // One-shot LIGHT_OUT reset handshake so host RGB takes over cleanly
      try {
        const init = new Uint8Array(47);
        init[38] = 0x02;
        init[41] = 0x02;
        await dev.sendReport(0x02, init);
      } catch {
        // Ignore if controller does not require reset
      }
      return true;
    } catch {
      return false;
    }
  }

  async disconnect(): Promise<void> {
    if (this.device) {
      try {
        if (this.device.opened) {
          await this.device.close();
        }
      } catch {
        // Ignore close errors
      }
      this.device = null;
    }
  }

  async sendReport(
    triggers: DualTriggerConfig,
    lighting: LightingConfig,
    heavyRumble = 0,
    lightRumble = 0
  ): Promise<boolean> {
    if (!this.device || !this.device.opened) return false;
    try {
      // 47-byte payload for USB Report ID 0x02 (without leading Report ID byte)
      const data = new Uint8Array(47);
      data[0] = 0x0f; // valid_flag0: rumble + triggers only (never overwrite audio volumes)
      data[1] = 0x15; // valid_flag1 (never set 0x08 RELEASE_LEDS)
      data[2] = lightRumble & 0xff;
      data[3] = heavyRumble & 0xff;
      data[8] = micLedByte(lighting.mic_led);

      data.set(compileTriggerPayload(triggers.right), 10);
      data.set(compileTriggerPayload(triggers.left), 21);

      data[38] = 0x00; // valid_flag2: MUST be 0x00 during normal RGB updates
      data[41] = 0x00; // lightbar_setup: MUST be 0x00 (never repeat LIGHT_OUT)
      data[42] = 0x00; // led_brightness high
      data[43] = playerLedBitmask(lighting.player_led);

      const rgb =
        lighting.lightbar.mode === 'static' || lighting.lightbar.mode === 'breathing'
          ? lighting.lightbar.color
          : lighting.lightbar.mode === 'off'
            ? { r: 0, g: 0, b: 0 }
            : { r: 0, g: 68, b: 204 };

      data[44] = rgb.r & 0xff;
      data[45] = rgb.g & 0xff;
      data[46] = rgb.b & 0xff;

      await this.device.sendReport(0x02, data);
      return true;
    } catch {
      return false;
    }
  }
}

export const webHidController = new WebHidDualSenseController();

export function pollStandardGamepad(): LiveGamepadState | null {
  if (typeof navigator === 'undefined' || !navigator.getGamepads) {
    return null;
  }
  const pads = navigator.getGamepads();
  const pad = Array.from(pads).find((gp): gp is Gamepad => gp !== null && gp.connected);
  if (!pad) return null;

  const pressed = (index: number) => Boolean(pad.buttons[index]?.pressed);
  const value = (index: number) => Math.round((pad.buttons[index]?.value ?? 0) * 100);

  return {
    connected: true,
    id: pad.id,
    leftStick: {
      x: pad.axes[0] ?? 0,
      y: pad.axes[1] ?? 0,
    },
    rightStick: {
      x: pad.axes[2] ?? 0,
      y: pad.axes[3] ?? 0,
    },
    l2: value(6),
    r2: value(7),
    buttons: {
      cross: pressed(0),
      circle: pressed(1),
      square: pressed(2),
      triangle: pressed(3),
      l1: pressed(4),
      r1: pressed(5),
      create: pressed(8),
      options: pressed(9),
      l3: pressed(10),
      r3: pressed(11),
      dpadUp: pressed(12),
      dpadDown: pressed(13),
      dpadLeft: pressed(14),
      dpadRight: pressed(15),
      ps: pressed(16),
      touchpad: pressed(17),
    },
  };
}

export async function triggerHapticRumble(
  durationMs: number,
  weakMagnitude: number,
  strongMagnitude: number
): Promise<boolean> {
  if (typeof navigator !== 'undefined' && navigator.getGamepads) {
    const pad = Array.from(navigator.getGamepads()).find(
      (gp): gp is Gamepad => gp !== null && gp.connected
    );
    const actuator = (
      pad as Gamepad & {
        vibrationActuator?: {
          playEffect?: (
            type: string,
            params: {
              duration: number;
              weakMagnitude: number;
              strongMagnitude: number;
            }
          ) => Promise<string>;
        };
      }
    )?.vibrationActuator;

    if (actuator?.playEffect) {
      try {
        await actuator.playEffect('dual-rumble', {
          duration: durationMs,
          weakMagnitude,
          strongMagnitude,
        });
        return true;
      } catch {
        // Fall through to WebHID
      }
    }
  }

  if (webHidController.isConnected()) {
    const defaultTriggers: DualTriggerConfig = {
      left: { mode: 'off' },
      right: { mode: 'off' },
    };
    const defaultLighting: LightingConfig = {
      lightbar: { mode: 'static', color: { r: 0, g: 68, b: 204 } },
      player_led: 'player1',
      mic_led: 'off',
    };
    const ok = await webHidController.sendReport(
      defaultTriggers,
      defaultLighting,
      Math.round(strongMagnitude * 255),
      Math.round(weakMagnitude * 255)
    );
    if (ok) {
      window.setTimeout(() => {
        void webHidController.sendReport(defaultTriggers, defaultLighting, 0, 0);
      }, durationMs);
      return true;
    }
  }

  return false;
}
