import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { TDataDetailZoneCommon } from '@/hooks'
import { isNumber, map } from 'lodash'
import { useNavigate } from 'react-router-dom'
import { TextCopy } from '../text-copy'

type Props = {
  item: TDataDetailZoneCommon
  hasLogs?: boolean
}

export const ZoneDetailCard: React.FC<Props> = ({ item, hasLogs }) => {
  const navigate = useNavigate()
  return (
    <Card className="shadow-xs">
      <CardContent className="grid gap-3 p-3">
        <div className="flex items-center justify-between">
          <div className="flex flex-row gap-y-2 text-xs">
            <div className="w-20 font-medium">node_id</div>
            <div className="flex-1">{item?.node_id}</div>
          </div>
          {hasLogs && (
            <DropdownMenu>
              <DropdownMenuTrigger className="text-xs underline">View Logs</DropdownMenuTrigger>
              <DropdownMenuContent>
                <DropdownMenuItem
                  onClick={() => {
                    navigate(`/zones/rooms?connector_id=${item?.node_id}`)
                  }}
                >
                  Logs Rooms
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => {
                    navigate(`/zones/sessions?connector_id=${item?.node_id}`)
                  }}
                >
                  Logs Sessions
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => {
                    navigate(`/zones/events?connector_id=${item?.node_id}`)
                  }}
                >
                  Logs Events
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>
        <div className="flex flex-col gap-y-2 text-xs lg:flex-row">
          <div className="w-20 font-medium">addr</div>
          <div className="flex-1">
            <TextCopy value={item?.addr} />
          </div>
        </div>
        <div className="flex flex-col gap-y-2 text-xs lg:flex-row">
          <div className="w-20 font-medium">conns</div>
          <div className="flex-1">
            <div className="flex flex-wrap items-center gap-x-2 gap-y-2">
              {map(item?.conns, (connect, cIdx) => (
                <div className="bg-muted flex items-center gap-1 rounded p-1" key={cIdx}>
                  addr: {connect.addr}, node: {connect.node}, rtt_ms: {connect.rtt_ms}
                </div>
              ))}
            </div>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="secondary" className="w-fit gap-2 rounded">
            <div>cpu:</div>
            <div>{item?.cpu}</div>
          </Badge>
          <Badge variant="secondary" className="w-fit gap-2 rounded">
            <div>disk:</div>
            <div>{item?.disk}</div>
          </Badge>
          <Badge variant="secondary" className="w-fit gap-2 rounded">
            <div>memory:</div>
            <div>{item?.memory}</div>
          </Badge>
          {isNumber(item?.live) && (
            <Badge variant="secondary" className="w-fit gap-2 rounded">
              <div>live:</div>
              <div>{item?.live}</div>
            </Badge>
          )}
          {isNumber(item?.max) && (
            <Badge variant="secondary" className="w-fit gap-2 rounded">
              <div>max:</div>
              <div>{item?.max}</div>
            </Badge>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
