import { Pagination } from '@/components'
import { Card, CardContent } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { useConnectorLogEventsQuery } from '@/hooks'
import { INITIAL_LIMIT, INITIAL_PAGE } from '@/utils'
import dayjs from 'dayjs'
import { isEmpty, map } from 'lodash'
import { useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'

export const ZonesEvents = () => {
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const connector_id = searchParams.get('connector_id') || ''

  if (!connector_id) navigate('/zones')

  const page = searchParams.get('page') ? Number(searchParams.get('page')) : INITIAL_PAGE
  const [limit, setLimit] = useState(INITIAL_LIMIT)
  const { data: events } = useConnectorLogEventsQuery({
    payload: {
      connector_id,
      page: page - 1,
      limit,
    },
    options: {
      enabled: !!connector_id,
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
    setSearchParams({ connector_id, page: String(events?.pagination?.total) })
  }

  const onChangeLimit = (value: number) => {
    setLimit(value)
    setSearchParams({ connector_id, page: '1' })
  }

  // TODO: Add loading UI
  return (
    <>
      <Card className="shadow-sm">
        <CardContent className="grid gap-2 p-3">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Event</TableHead>
                <TableHead>Session</TableHead>
                <TableHead>Node ID</TableHead>
                <TableHead className="text-right">Node Timestamp</TableHead>
                <TableHead className="text-right">Created At</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!isEmpty(events?.data) ? (
                map(events?.data, (e) => (
                  <TableRow key={e?.id}>
                    <TableCell>{e?.id}</TableCell>
                    <TableCell>{e?.event}</TableCell>
                    <TableCell>{e?.session}</TableCell>
                    <TableCell>{e?.node}</TableCell>
                    <TableCell className="whitespace-nowrap text-right">
                      <div>
                        <p>{e?.node_ts ? dayjs(e?.node_ts).format('ll') : '---'}</p>
                        <p>{e?.node_ts ? dayjs(e?.node_ts).format('LT') : '---'}</p>
                      </div>
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-right">
                      <div>
                        <p>{e?.created_at ? dayjs(e?.created_at).format('ll') : '---'}</p>
                        <p>{e?.created_at ? dayjs(e?.created_at).format('LT') : '---'}</p>
                      </div>
                    </TableCell>
                  </TableRow>
                ))
              ) : (
                <TableRow>
                  <TableCell colSpan={6} className="text-center">
                    <span className="text-muted-foreground">No events found</span>
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
          pagination={events?.pagination}
          limit={limit}
          setLimit={onChangeLimit}
        />
      </div>
    </>
  )
}
