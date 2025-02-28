import { Pagination } from '@/components'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { useConnectorLogRoomsQuery } from '@/hooks'
import { INITIAL_LIMIT, INITIAL_PAGE } from '@/utils'
import dayjs from 'dayjs'
import LocalizedFormat from 'dayjs/plugin/localizedFormat'
import { isEmpty, map } from 'lodash'
import { useState } from 'react'
import { Link, useNavigate, useSearchParams } from 'react-router-dom'

dayjs.extend(LocalizedFormat)

export const ZonesRooms = () => {
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const connector_id = searchParams.get('connector_id') || ''

  if (!connector_id) navigate('/zones')

  const page = searchParams.get('page') ? Number(searchParams.get('page')) : INITIAL_PAGE
  const [limit, setLimit] = useState(INITIAL_LIMIT)
  const { data: rooms } = useConnectorLogRoomsQuery({
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
    setSearchParams({ connector_id, page: String(page - 1) })
  }

  const onNext = () => {
    setSearchParams({ connector_id, page: String(page + 1) })
  }

  const onFirst = () => {
    setSearchParams({ connector_id, page: '1' })
  }

  const onLast = () => {
    setSearchParams({ connector_id, page: String(rooms?.pagination?.total) })
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
                <TableHead>ID</TableHead>
                <TableHead>App</TableHead>
                <TableHead>Room</TableHead>
                <TableHead>Peers</TableHead>
                <TableHead>Record</TableHead>
                <TableHead className="text-right">Created At</TableHead>
                <TableHead className="w-[102px]" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {!isEmpty(rooms?.data) ? (
                map(rooms?.data, (r) => (
                  <TableRow key={r?.id}>
                    <TableCell>{r?.id}</TableCell>
                    <TableCell>{r?.app}</TableCell>
                    <TableCell>{r?.room}</TableCell>
                    <TableCell>{r?.peers}</TableCell>
                    <TableCell>{r?.record}</TableCell>
                    <TableCell className="text-right whitespace-nowrap">
                      <div>
                        <p>{r?.created_at ? dayjs(r?.created_at).format('ll') : '---'}</p>
                        <p>{r?.created_at ? dayjs(r?.created_at).format('LT') : '---'}</p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <Link to={`/zones/peers?connector_id=${connector_id}&room_id=${r?.id}`}>
                        <Button variant="ghost" className="h-8 items-center text-xs underline">
                          View details
                        </Button>
                      </Link>
                    </TableCell>
                  </TableRow>
                ))
              ) : (
                <TableRow>
                  <TableCell colSpan={6} className="text-center">
                    <span className="text-muted-foreground">No rooms found</span>
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
          pagination={rooms?.pagination}
          limit={limit}
          setLimit={onChangeLimit}
        />
      </div>
    </>
  )
}
