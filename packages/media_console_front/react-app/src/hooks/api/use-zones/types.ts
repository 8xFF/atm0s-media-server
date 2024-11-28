export type TConsoles = {
  data?: TDataDetailZoneCommon[]
  error?: string
  status: boolean
}

export type TDataZone = {
  connectors: number
  consoles: number
  gateways: number
  lat: number
  lon: number
  medias: number
  zone_id: number
}

export type TZones = {
  data?: TDataZone[]
  error?: string
  status: boolean
}

export type TDataDetailZoneConns = {
  addr: string
  node: number
  rtt_ms: number
}

export type TDataDetailZoneCommon = {
  addr: string
  conns: TDataDetailZoneConns[]
  cpu: number
  disk: number
  memory: number
  node_id: number
  live?: number
  max?: number
}

export type TDataDetailZone = {
  connectors: TDataDetailZoneCommon[]
  consoles: TDataDetailZoneCommon[]
  gateways: TDataDetailZoneCommon[]
  medias: TDataDetailZoneCommon[]
  lat: number
  lon: number
}

export type TZone = {
  data?: TDataDetailZone
  error?: string
  status: boolean
}
