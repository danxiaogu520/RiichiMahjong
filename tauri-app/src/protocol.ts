export type PlayerId = number;

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

export interface GameStateView {
  players: PlayerView[];
  wind: number;
  round: number;
  honba: number;
  riichi_sticks: number;
  dora: number[];
  remaining_tiles: number;
  phase: GamePhaseView;
}

export type GamePhaseView =
  | { DrawPhase: { player: PlayerId; position: "LiveWall" | "Rinshan" } }
  | { ActionPhase: { player: PlayerId; drawn_tile: number | null } }
  | { ResponsePhase: { player: PlayerId; discarded_tile: number } }
  | { ChankanResponse: { player: PlayerId; kan_tile: number } }
  | "RoundOver";

export interface PlayerView {
  id: PlayerId;
  hand: number[] | null;
  hand_count: number;
  points: number;
  wind: number;
  discards: number[];
  melds: MeldView[];
  is_riichi: boolean;
  riichi_declaration_tile: number | null;
}

export interface MeldView { kind: string; tiles: number[]; from_player: PlayerId | null; }

export interface ActionRequest {
  player: PlayerId;
  can_tsumo: boolean;
  can_riichi: boolean;
  riichi_options: number[];
  discard_options: number[];
  ankan_options: number[];
  kakan_options: [number, number][];
  can_kyuushu: boolean;
}

export interface CallRequest {
  player: PlayerId;
  options: { player: PlayerId; call_type: Record<string, unknown> }[];
}

export function playerIndex(player: PlayerId): number {
  return player;
}
