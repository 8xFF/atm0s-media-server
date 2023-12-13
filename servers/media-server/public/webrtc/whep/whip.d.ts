export declare class WHIPClient {
    constructor();
    publish(pc: RTCPeerConnection, url: string, token?: string): Promise<void>;
    restart(): void;
    trickle(): Promise<void>;
    mute(muted: any): Promise<void>;
    stop(): Promise<void>;
}