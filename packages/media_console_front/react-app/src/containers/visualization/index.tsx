import { TNetworkEvent, useNetworkVisualization } from '@/hooks'
import { useCallback, useEffect, useRef } from 'react'
import { NetworkVisualizationGraph } from './graph'

export const NetworkVisualization = () => {
  const ref = useRef<HTMLDivElement | null>(null)
  const graph = useRef(new NetworkVisualizationGraph())
  const cb = useCallback((data: TNetworkEvent) => {
    const ctx = graph.current
    ctx.onEvent(data)
  }, [])
  useNetworkVisualization(cb)

  useEffect(() => {
    const ctx = graph.current
    if (ref.current) ctx.init(ref.current)
  }, [])

  return (
    <>
      <div ref={ref} style={{ width: '100%', height: '100%' }}></div>
    </>
  )
}
