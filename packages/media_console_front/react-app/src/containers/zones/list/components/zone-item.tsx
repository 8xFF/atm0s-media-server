import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { TDataZone } from '@/hooks'
import { SignalIcon } from 'lucide-react'
import { useNavigate } from 'react-router-dom'

type Props = {
  zone: TDataZone
}

export const ZoneItem: React.FC<Props> = ({ zone }) => {
  const navigate = useNavigate()
  return (
    <Card className="cursor-pointer shadow-sm" onClick={() => navigate(`/zones/${zone?.zone_id}`)}>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 p-3 pb-0">
        <CardTitle className="text-sm font-medium">zone_id: {zone?.zone_id}</CardTitle>
        <SignalIcon className="text-emerald-500" size={16} />
      </CardHeader>
      <CardContent className="p-3">
        <div className="grid gap-2">
          <div className="flex items-center justify-between text-sm text-muted-foreground">
            <div>consoles</div>
            <div>{zone?.consoles}</div>
          </div>
          <div className="flex items-center justify-between text-sm text-muted-foreground">
            <div>gateways</div>
            <div>{zone?.gateways}</div>
          </div>
          <div className="flex items-center justify-between text-sm text-muted-foreground">
            <div>medias</div>
            <div>{zone?.medias}</div>
          </div>
          <div className="flex items-center justify-between text-sm text-muted-foreground">
            <div>connectors</div>
            <div>{zone?.connectors}</div>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
