export type PlayerId = number;

export interface RoomPlayerView {
  id: PlayerId;
  nickname: string;
  ready: boolean;
  connected: boolean;
  is_ai: boolean;
  ai_takeover: boolean;
}

export interface RoomInfo {
  id: string;
  owner: PlayerId | null;
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

export interface PlayerControllerChanged {
  player_id: PlayerId;
  is_ai: boolean;
  ai_takeover: boolean;
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
  tenpai_info: TenpaiInfoView | null;
  analysis: AnalysisInfo | null;
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
  /** 立直宣言牌在 discards 中的下标（宣言牌被鸣走时为立直后第一张入河的牌） */
  riichi_declaration_index: number | null;
}

export interface MeldView {
  kind: string;
  tiles: number[];
  from_player: PlayerId | null;
}

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

// ─── 服务端事件（Event 消息体） ───────────────────────────────

export type GameEventView =
  | { Draw: { player: PlayerId; tile: number | null } }
  | { Discard: { player: PlayerId; tile: number; kind: "Tsumogiri" | "Tedashi" } }
  | {
      Call: {
        player: PlayerId;
        kind: "Chi" | "Pon" | "Minkan" | "Ankan" | "Kakan";
        tiles: number[];
        called_tile: number | null;
        from_player: PlayerId | null;
        meld_index: number | null;
      };
    }
  | { Pass: { player: PlayerId } }
  | { Riichi: { player: PlayerId } }
  | {
      Win: {
        winners: PlayerId[];
        tile: number;
        kind: "Ron" | "Tsumo";
        loser: PlayerId | null;
      };
    }
  | { AbortiveDraw: { player: PlayerId | null; reason: RoundEndReasonView } };

export interface GameEventEnvelope {
  event_id: number;
  event: GameEventView;
}

export type RoundEndReasonView =
  | { ExhaustiveDraw: null }
  | "ExhaustiveDraw"
  | { Win: { winner: PlayerId; is_tsumo: boolean } }
  | { MultiWin: { winners: PlayerId[] } }
  | { KyuushuKyuuhai: null }
  | "KyuushuKyuuhai"
  | { SuufonRenda: null }
  | "SuufonRenda"
  | { SuuchaRiichi: null }
  | "SuuchaRiichi"
  | { SuuKantsu: null }
  | "SuuKantsu"
  | { Unknown: string };

export interface RoundResultView {
  reason: RoundEndReasonView;
  win_details: string[];
  point_changes: [number, number, number, number];
}

export interface GameOverView {
  scores: [number, number, number, number];
  ranking: [number, number, number, number];
}

// ─── 听牌与牌效分析 ──────────────────────────────────────────

export interface TenpaiInfoView {
  waits: WaitInfoView[];
  is_furiten: boolean;
}

export interface WaitInfoView {
  tile_type: number;
  remaining: number;
  is_no_yaku: boolean;
}

export interface AnalysisInfo {
  discard_options: DiscardOptionView[];
  acceptance: AcceptanceInfoView[];
  improvement: AcceptanceInfoView[];
  current_shanten: number;
}

export interface DiscardOptionView {
  tile: number;
  shanten: number;
  acceptance_count: number;
  improvement_count: number;
}

export interface AcceptanceInfoView {
  tile_type: number;
  copies: number;
}

export function playerIndex(player: PlayerId): number {
  return player;
}
