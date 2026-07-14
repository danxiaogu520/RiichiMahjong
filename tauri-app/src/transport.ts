import type {
  ClientEnvelope,
  ClientMessage,
  JoinInfo,
  RoomInfo,
  ServerEnvelope,
} from "./protocol";

const PROTOCOL_VERSION = 1;

export interface TransportCallbacks {
  onMessage: (message: ServerEnvelope) => void;
  onClose: () => void;
  onError: (message: string) => void;
}

export class ClientTransport {
  private readonly httpOrigin: string;
  private socket: WebSocket | undefined;
  private commandId = 0;
  private sequence = 0;

  constructor(serverAddress: string) {
    this.httpOrigin = serverAddress.replace(/\/$/, "");
  }

  async createRoom(): Promise<RoomInfo> {
    return this.request<RoomInfo>("/rooms", { method: "POST" });
  }

  async joinRoom(roomId: string, nickname: string): Promise<JoinInfo> {
    return this.request<JoinInfo>(`/rooms/${encodeURIComponent(roomId)}/join`, {
      method: "POST",
      body: JSON.stringify({ nickname }),
    });
  }

  async setReady(roomId: string, token: string, ready: boolean): Promise<RoomInfo> {
    return this.request<RoomInfo>(`/rooms/${encodeURIComponent(roomId)}/ready`, {
      method: "POST",
      body: JSON.stringify({ token, ready }),
    });
  }

  async startRoom(roomId: string, token: string): Promise<RoomInfo> {
    return this.request<RoomInfo>(`/rooms/${encodeURIComponent(roomId)}/start`, {
      method: "POST",
      body: JSON.stringify({ token }),
    });
  }

  connect(roomId: string, token: string, callbacks: TransportCallbacks): void {
    // 每条 WebSocket 连接都有自己的服务端序号；重连必须从快照重新建立序列。
    this.sequence = 0;
    const url = new URL(this.httpOrigin);
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    url.pathname = "/ws";
    url.search = new URLSearchParams({ room_id: roomId, token }).toString();
    this.socket = new WebSocket(url);
    this.socket.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data) as ServerEnvelope;
        if (typeof message.seq === "number" && message.seq > this.sequence + 1 && this.sequence !== 0) {
          callbacks.onError("检测到消息序号缺失，正在请求完整快照");
          this.requestSnapshot();
        }
        if (typeof message.seq === "number") {
          this.sequence = message.seq;
        }
        callbacks.onMessage(message);
      } catch {
        callbacks.onError("服务器消息格式无效");
      }
    };
    this.socket.onclose = callbacks.onClose;
    this.socket.onerror = () => callbacks.onError("WebSocket 连接失败");
  }

  send(body: ClientMessage): void {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      throw new Error("WebSocket 尚未连接");
    }
    const envelope: ClientEnvelope = {
      protocol_version: PROTOCOL_VERSION,
      command_id: ++this.commandId,
      expected_seq: this.sequence,
      body,
    };
    this.socket.send(JSON.stringify(envelope));
  }

  requestSnapshot(): void {
    this.send({ RequestSnapshot: null });
  }

  close(): void {
    this.socket?.close();
    this.socket = undefined;
  }

  private async request<T>(path: string, init: RequestInit): Promise<T> {
    const response = await fetch(`${this.httpOrigin}${path}`, {
      ...init,
      headers: { "content-type": "application/json", ...init.headers },
    });
    if (!response.ok) {
      throw new Error((await response.text()) || `请求失败（${response.status}）`);
    }
    return (await response.json()) as T;
  }
}
