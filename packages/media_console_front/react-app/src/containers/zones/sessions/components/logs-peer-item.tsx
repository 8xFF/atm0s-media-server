import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { TDataConnectorLogSessions } from '@/hooks'
import dayjs from 'dayjs'
import { map } from 'lodash'
import { MinusIcon, PlusIcon } from 'lucide-react'
import { Fragment, useState } from 'react'

type Props = {
  session: TDataConnectorLogSessions
}

export const LogsPeerItem: React.FC<Props> = ({ session }) => {
  const [expanded, setExpanded] = useState(false)
  return (
    <>
      <TableRow className="cursor-pointer" onClick={() => setExpanded(!expanded)}>
        <TableCell>
          {!expanded ? (
            <PlusIcon className="text-muted-foreground" size={16} />
          ) : (
            <MinusIcon className="text-muted-foreground" size={16} />
          )}
        </TableCell>
        <TableCell>{session?.id}</TableCell>
        <TableCell>{session?.app}</TableCell>
        <TableCell>{session?.ip}</TableCell>
        <TableCell>{session?.sdk}</TableCell>
        <TableCell>{session?.user_agent}</TableCell>
        <TableCell className="whitespace-nowrap text-right">
          <div>
            <p>{session?.created_at ? dayjs(session?.created_at).format('ll') : '---'}</p>
            <p>{session?.created_at ? dayjs(session?.created_at).format('LT') : '---'}</p>
          </div>
        </TableCell>
      </TableRow>
      {expanded && (
        <TableRow>
          <TableCell colSpan={7} className="bg-muted">
            <Card className="shadow-sm">
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-0">
                <CardTitle className="text-base font-medium">Sessions</CardTitle>
              </CardHeader>
              <CardContent className="grid gap-2 p-3">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>ID</TableHead>
                      <TableHead>Session</TableHead>
                      <TableHead className="text-right">Created At</TableHead>
                      <TableHead className="text-right">Joined At</TableHead>
                      <TableHead className="text-right">Leaved At</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {map(session?.sessions, (s) => (
                      <Fragment key={s?.id}>
                        <TableRow>
                          <TableCell>{s?.id}</TableCell>
                          <TableCell>{s?.session}</TableCell>
                          <TableCell className="whitespace-nowrap text-right">
                            <div>
                              <p>{s?.created_at ? dayjs(s?.created_at).format('ll') : '---'}</p>
                              <p>{s?.created_at ? dayjs(s?.created_at).format('LT') : '---'}</p>
                            </div>
                          </TableCell>
                          <TableCell className="whitespace-nowrap text-right">
                            <div>
                              <p>{s?.joined_at ? dayjs(s?.joined_at).format('ll') : '---'}</p>
                              <p>{s?.joined_at ? dayjs(s?.joined_at).format('LT') : '---'}</p>
                            </div>
                          </TableCell>
                          <TableCell className="whitespace-nowrap text-right">
                            <div>
                              <p>{s?.leaved_at ? dayjs(s?.leaved_at).format('ll') : '---'}</p>
                              <p>{s?.leaved_at ? dayjs(s?.leaved_at).format('LT') : '---'}</p>
                            </div>
                          </TableCell>
                        </TableRow>
                      </Fragment>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </TableCell>
        </TableRow>
      )}
    </>
  )
}
