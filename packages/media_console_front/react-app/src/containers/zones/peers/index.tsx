import { Pagination } from '@/components'
import { Card, CardContent } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { useConnectorLogPeersQuery } from '@/hooks'
import { INITIAL_LIMIT, INITIAL_PAGE } from '@/utils'
import { isEmpty, map } from 'lodash'
import { useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { LogsPeerItem } from './components'

export const ZonesPeers = () => {
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const connector_id = searchParams.get('connector_id') || ''
  const room_id = searchParams.get('room_id') || ''

  if (!connector_id) navigate('/zones')

  const page = searchParams.get('page') ? Number(searchParams.get('page')) : INITIAL_PAGE
  const [limit, setLimit] = useState(INITIAL_LIMIT)
  const { data: peers } = useConnectorLogPeersQuery({
    payload: {
      connector_id,
      room_id,
      page: page - 1,
      limit,
    },
    options: {
      enabled: !!connector_id && !!room_id,
    },
  })

  const onPrev = () => {
    setSearchParams({
      connector_id,
      page: String(page - 1),
    })
  }

  const onNext = () => {
    setSearchParams({ connector_id, page: String(page + 1) })
  }

  const onFirst = () => {
    setSearchParams({ connector_id, page: '1' })
  }

  const onLast = () => {
    setSearchParams({ connector_id, page: String(peers?.pagination?.total) })
  }

  const onChangeLimit = (value: number) => {
    setLimit(value)
    setSearchParams({ connector_id, page: '1' })
  }

  // TODO: Add loading UI
  return (
    <>
      <Card className="shadow-xs">
        <CardContent className="grid gap-2 p-3">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-6" />
                <TableHead>ID</TableHead>
                <TableHead>Peer</TableHead>
                <TableHead>Room ID</TableHead>
                <TableHead>Room</TableHead>
                <TableHead className="text-right">Created At</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!isEmpty(peers?.data) ? (
                map(peers?.data, (p) => <LogsPeerItem peer={p} key={p?.id} />)
              ) : (
                <TableRow>
                  <TableCell colSpan={6} className="text-center">
                    <span className="text-muted-foreground">No peers found</span>
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      <div className="sticky bottom-0 bg-white p-6">
        <Pagination
          onFirst={onFirst}
          onLast={onLast}
          onPrev={onPrev}
          onNext={onNext}
          pagination={peers?.pagination}
          limit={limit}
          setLimit={onChangeLimit}
        />
      </div>
    </>
  )
}
