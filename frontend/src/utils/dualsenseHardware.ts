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
        return false;
      }
    }
  }

  return false;
}
