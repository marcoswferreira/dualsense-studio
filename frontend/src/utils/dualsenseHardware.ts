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

function compileTriggerPayload(effect: TriggerEffect): Uint8Array {
  const bytes = new Uint8Array(11);
  switch (effect.mode) {
    case 'off':
      bytes[0] = 0x05;
      break;
    case 'feedback':
      bytes[0] = 0x21;
      bytes[1] = Math.min(9, effect.start_position);
      bytes[2] = Math.min(8, effect.force);
      break;
    case 'weapon':
      bytes[0] = 0x25;
      bytes[1] = Math.min(9, effect.start_position);
      bytes[2] = Math.min(9, effect.end_position);
      bytes[3] = Math.min(8, effect.force);
      break;
    case 'bow':
      bytes[0] = 0x22;
      bytes[1] = Math.min(9, effect.start_position);
      bytes[2] = Math.min(9, effect.end_position);
      bytes[3] = Math.min(8, effect.force);
      bytes[4] = Math.min(8, effect.snap_force);
      break;
    case 'galloping':
      bytes[0] = 0x23;
      bytes[1] = Math.min(9, effect.start_position);
      bytes[2] = Math.min(9, effect.end_position);
      bytes[3] = Math.min(9, effect.first_foot);
      bytes[4] = Math.min(9, effect.second_foot);
      bytes[5] = effect.frequency & 0xff;
      break;
    case 'machine_gun':
      bytes[0] = 0x26;
      bytes[1] = Math.min(9, effect.start_position);
      bytes[2] = Math.min(9, effect.end_position);
      bytes[3] = Math.min(8, effect.strength);
      bytes[4] = effect.frequency & 0xff;
      break;
    case 'machine':
      bytes[0] = 0x27;
      bytes[1] = Math.min(9, effect.start_position);
      bytes[2] = Math.min(9, effect.end_position);
      bytes[3] = Math.min(7, effect.strength_a);
      bytes[4] = Math.min(7, effect.strength_b);
      bytes[5] = effect.frequency & 0xff;
      bytes[6] = effect.period & 0xff;
      break;
    case 'vibration':
      bytes[0] = 0x06;
      bytes[1] = effect.frequency & 0xff;
      bytes[2] = Math.min(63, effect.amplitude);
      bytes[3] = Math.min(9, effect.position);
      break;
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
    case 'custom_zone':
      bytes[0] = 0x27;
      for (let i = 0; i < 10; i++) {
        bytes[i + 1] = (effect.zones[i] ?? 0) & 0xff;
      }
      break;
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
      data[0] = 0xff; // valid_flag0
      data[1] = 0x15; // valid_flag1 (never set 0x08 RELEASE_LEDS)
      data[2] = lightRumble & 0xff;
      data[3] = heavyRumble & 0xff;
      data[8] = micLedByte(lighting.mic_led);

      data.set(compileTriggerPayload(triggers.right), 10);
      data.set(compileTriggerPayload(triggers.left), 21);

      data[38] = 0x02; // valid_flag2
      data[41] = 0x02; // lightbar_setup
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
