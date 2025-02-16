import { useEffect } from 'react'
import { networkEventSchema, TNetworkEvent } from './types'

export * from './types'

export type NetworkEventCallback = (data: TNetworkEvent) => void

export const useNetworkVisualization = (cb?: NetworkEventCallback, url?: string) => {
  useEffect(() => {
    const wsUrl =
      url || (window.location.protocol === 'https:' ? 'wss' : 'ws') + '://' + window.location.host + '/ws/network'
    const wsClient = new WebSocket(wsUrl)

    wsClient.onopen = () => {
      console.log('WebSocket Client Connected')
    }
    wsClient.onmessage = (message) => {
      try {
        const object = JSON.parse(message.data)
        const event = networkEventSchema.parse(object)
        if (cb) {
          cb(event)
        }
      } catch (e) {
        console.error('[network] error when parser msg', e)
      }
    }

    return () => {
      console.log('on websocket close')
      wsClient.close()
    }
  }, [])
}
