export type TDataConnectorLogRooms = {
  id: number
  app: string
  room: string
  peers: number
  created_at: number
  record?: string
}

export type TConnectorLogRooms = {
  data?: TDataConnectorLogRooms[]
  pagination?: {
    current: number
    total: number
  }
  error?: string
  status: boolean
}

export type TDataConnectorLogPeers = {
  created_at: number
  id: number
  peer: string
  room: string
  room_id: number
  sessions: {
    created_at: number
    id: number
    joined_at: number
    leaved_at?: number
    peer: string
    peer_id: number
    session: string
  }[]
}

export type TConnectorLogPeers = {
  data?: TDataConnectorLogPeers[]
  pagination?: {
    current: number
    total: number
  }
  error?: string
  status: boolean
}

export type TDataConnectorLogSessions = {
  created_at: number
  app: string
  id: string
  ip?: string
  sdk: any
  user_agent: any
  sessions: {
    created_at: number
    id: number
    joined_at: number
    leaved_at?: number
    peer: string
    peer_id: number
    session: string
  }[]
}

export type TConnectorLogSessions = {
  data?: TDataConnectorLogSessions[]
  pagination?: {
    current: number
    total: number
  }
  error?: string
  status: boolean
}

export type TDataConnectorLogEvents = {
  created_at: number
  event: string
  id: number
  meta: string
  node: number
  node_ts: number
  session: string
}

export type TConnectorLogEvents = {
  data?: TDataConnectorLogEvents[]
  pagination?: {
    current: number
    total: number
  }
  error?: string
  status: boolean
}
