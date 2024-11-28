import { useApi } from '@/hooks'
import { TInputQuery } from '@/types'
import { useQuery } from '@tanstack/react-query'
import { TConsoles, TZone, TZones } from './types'

export * from './types'

export const useConsolesQuery = ({ options }: TInputQuery<{}, TConsoles>) => {
  const { api } = useApi()

  const fetcher = async () => {
    const rs = await api.get('/cluster/consoles')
    return rs.data
  }

  return useQuery({
    queryKey: ['useConsolesQuery'],
    queryFn: fetcher,
    retry: false,
    ...options,
  })
}

export const useZonesQuery = ({ options }: TInputQuery<{}, TZones>) => {
  const { api } = useApi()

  const fetcher = async () => {
    const rs = await api.get('/cluster/zones')
    return rs.data
  }

  return useQuery({
    queryKey: ['useZonesQuery'],
    queryFn: fetcher,
    retry: false,
    refetchInterval: 60000,
    ...options,
  })
}

export const useDetailZoneQuery = ({
  payload,
  options,
}: TInputQuery<
  {
    zone_id?: string | null
  },
  TZone
>) => {
  const { api } = useApi()

  const fetcher = async () => {
    const rs = await api.get(`/cluster/zones/${payload?.zone_id}`)
    return rs.data
  }

  return useQuery({
    queryKey: ['useZoneQuery'],
    queryFn: fetcher,
    retry: false,
    refetchInterval: 60000,
    ...options,
  })
}
