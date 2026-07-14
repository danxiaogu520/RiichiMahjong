export type PlayerId = [number];

export interface RoomPlayerView {
  id: PlayerId;
  nickname: string;
  ready: boolean;
  connected: boolean;
}

export interface RoomInfo {
  id: string;
  players: RoomPlayerView[];
  started: boolean;
}

export interface JoinInfo {
  room: RoomInfo;
  player: PlayerId;
  token: string;
}

export type ClientMessage =
  | { JoinRoom: { room_id: string } }
  | { RequestSnapshot: null }
  | { TurnAction: { action: TurnActionPayload } }
  | { CallResponse: { action: CallResponsePayload } }
  | { Ready: null }
  | { LeaveRoom: null };

export type TurnActionPayload =
  | { Discard: number }
  | { RiichiDiscard: number }
  | { Tsumo: null }
  | { Ankan: number }
  | { Kakan: [number, number] }
  | { KyuushuKyuuhai: null };

export type CallResponsePayload =
  | { Pass: null }
  | { Ron: null }
  | { Pon: { hand_tiles: [number, number] } }
  | { Chi: { hand_tiles: [number, number] } }
  | { Minkan: { hand_tiles: [number, number, number] } };

export interface ClientEnvelope {
  protocol_version: number;
  command_id: number;
  expected_seq: number;
  body: ClientMessage;
}

export interface ServerEnvelope {
  protocol_version: number;
  seq: number;
  body: Record<string, unknown>;
}

export function playerIndex(player: PlayerId): number {
  return player[0];
}
