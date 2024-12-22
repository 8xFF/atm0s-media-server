import { Pagination } from '@/components'
import { Card, CardContent } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { useConnectorLogSessionsQuery } from '@/hooks'
import { Layout } from '@/layouts'
import { INITIAL_LIMIT, INITIAL_PAGE } from '@/utils'
import dayjs from 'dayjs'
import LocalizedFormat from 'dayjs/plugin/localizedFormat'
import { isEmpty, map } from 'lodash'
import { useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { LogsPeerItem } from './components'

dayjs.extend(LocalizedFormat)

export const ZonesSessions = () => {
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const connector_id = searchParams.get('connector_id') || ''

  if (!connector_id) navigate('/zones')

  const page = searchParams.get('page') ? Number(searchParams.get('page')) : INITIAL_PAGE
  const [limit, setLimit] = useState(INITIAL_LIMIT)
  const { data: sessions } = useConnectorLogSessionsQuery({
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
    setSearchParams({ connector_id, page: String(sessions?.pagination?.total) })
  }

  const onChangeLimit = (value: number) => {
    setLimit(value)
    setSearchParams({ connector_id, page: '1' })
  }

  // TODO: Add loading UI
  return (
    <Layout>
      <Card className="shadow-sm">
        <CardContent className="grid gap-2 p-3">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-6" />
                <TableHead>ID</TableHead>
                <TableHead>App</TableHead>
                <TableHead>IP</TableHead>
                <TableHead>SDK</TableHead>
                <TableHead>User Agent</TableHead>
                <TableHead className="text-right">Created At</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!isEmpty(sessions?.data) ? (
                map(sessions?.data, (s) => <LogsPeerItem session={s} key={s?.id} />)
              ) : (
                <TableRow>
                  <TableCell colSpan={6} className="text-center">
                    <span className="text-muted-foreground">No sessions found</span>
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
          pagination={sessions?.pagination}
          limit={limit}
          setLimit={onChangeLimit}
        />
      </div>
    </Layout>
  )
}
