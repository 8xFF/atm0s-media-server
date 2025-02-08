import { useApi } from '@/hooks'
import { TInputQuery } from '@/types'
import { useQuery } from '@tanstack/react-query'
import { TConnectorLogEvents, TConnectorLogPeers, TConnectorLogRooms, TConnectorLogSessions } from './types'

export * from './types'

export const useConnectorLogRoomsQuery = ({
  payload,
  options,
}: TInputQuery<
  {
    connector_id?: string | null
    page: number
    limit: number
  },
  TConnectorLogRooms
>) => {
  const { api } = useApi()

  const fetcher = async () => {
    const rs = await api.get(`/connector/${payload?.connector_id}/log/rooms`, {
      params: {
        page: payload?.page,
        limit: payload?.limit,
      },
    })
    return rs.data
  }

  return useQuery({
    queryKey: ['useConnectorLogRoomsQuery', payload],
    queryFn: fetcher,
    retry: false,
    refetchInterval: 30000,
    ...options,
  })
}

export const useConnectorLogPeersQuery = ({
  payload,
  options,
}: TInputQuery<
  {
    connector_id?: string | null
    room_id?: string | null
    page: number
    limit: number
  },
  TConnectorLogPeers
>) => {
  const { api } = useApi()

  const fetcher = async () => {
    const rs = await api.get(`/connector/${payload?.connector_id}/log/peers`, {
      params: {
        room: payload?.room_id,
        page: payload?.page,
        limit: payload?.limit,
      },
    })
    return rs.data
  }

  return useQuery({
    queryKey: ['useConnectorLogPeersQuery', payload],
    queryFn: fetcher,
    retry: false,
    refetchInterval: 30000,
    ...options,
  })
}

export const useConnectorLogSessionsQuery = ({
  payload,
  options,
}: TInputQuery<
  {
    connector_id?: string | null
    room_id?: string | null
    page: number
    limit: number
  },
  TConnectorLogSessions
>) => {
  const { api } = useApi()

  const fetcher = async () => {
    const rs = await api.get(`/connector/${payload?.connector_id}/log/sessions`, {
      params: {
        room_id: payload?.room_id,
        page: payload?.page,
        limit: payload?.limit,
      },
    })
    return rs.data
  }

  return useQuery({
    queryKey: ['useConnectorLogSessionsQuery', payload],
    queryFn: fetcher,
    retry: false,
    refetchInterval: 30000,
    ...options,
  })
}

export const useConnectorLogEventsQuery = ({
  payload,
  options,
}: TInputQuery<
  {
    connector_id?: string | null
    page: number
    limit: number
  },
  TConnectorLogEvents
>) => {
  const { api } = useApi()

  const fetcher = async () => {
    const rs = await api.get(`/connector/${payload?.connector_id}/log/events`, {
      params: {
        page: payload?.page,
        limit: payload?.limit,
      },
    })
    return rs.data
  }

  return useQuery({
    queryKey: ['useConnectorLogEventsQuery', payload],
    queryFn: fetcher,
    retry: false,
    refetchInterval: 30000,
    ...options,
  })
}
