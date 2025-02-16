import { z } from 'zod'

export const networkNodeGenericInfo = z.object({
  addr: z.string().min(1),
  cpu: z.number(),
  memory: z.number(),
  disk: z.number(),
})

export const networkNodeConnectionInfo = z
  .object({
    id: z.number().int(),
    node: z.number().int(),
    addr: z.string().min(1),
    rtt_ms: z.number(),
  })
  .transform((data) => ({
    id: data.id,
    node: data.node,
    addr: data.addr,
    rttMs: data.rtt_ms,
  }))

export const networkNodeDataSchema = z
  .object({
    id: z.number().int(),
    zone_id: z.number().int(),
    info: networkNodeGenericInfo,
    conns: z.array(networkNodeConnectionInfo),
  })
  .transform((data) => ({
    id: data.id,
    zoneId: data.zone_id,
    info: data.info,
    connections: data.conns,
  }))

export const networkSnapShotEvent = z.object({
  type: z.literal('snapshot'),
  data: z.array(networkNodeDataSchema),
})

export const networkOnNodeChangedEvent = z.object({
  type: z.literal('on_changed'),
  data: networkNodeDataSchema,
})

export const networkOnRemovedEvent = z.object({
  type: z.literal('on_removed'),
  data: z.array(z.number().int()),
})

export const networkEventSchema = z.discriminatedUnion('type', [
  networkSnapShotEvent,
  networkOnNodeChangedEvent,
  networkOnRemovedEvent,
])

export type TNetworkConnectionInfo = z.infer<typeof networkNodeConnectionInfo>
export type TNetworkNodeGenericInfo = z.infer<typeof networkNodeGenericInfo>
export type TNetworkNodeData = z.infer<typeof networkNodeDataSchema>
export type TNetworkEvent = z.infer<typeof networkEventSchema>
