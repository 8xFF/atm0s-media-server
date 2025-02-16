import { TNetworkEvent, useNetworkVisualization } from '@/hooks'
import { Layout } from '@/layouts'
import { ThemeProviderContext } from '@/providers'
import { useCallback, useContext, useEffect, useRef } from 'react'
import { NetworkVisualizationGraph } from './graph'

export const NetworkVisualization = () => {
  const theme = useContext(ThemeProviderContext)

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
      <Layout>
        <div ref={ref} style={{ width: '100%', height: '100%' }}></div>
      </Layout>
    </>
  )
}
