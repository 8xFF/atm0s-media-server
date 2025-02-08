import { TDataDetailZoneCommon } from '@/hooks'
import { map } from 'lodash'
import { ZoneDetailCard } from './zone-detail-card'

type Props = {
  title: string
  data?: TDataDetailZoneCommon[]
  hasLogs?: boolean
}

export const ZoneDetailSection: React.FC<Props> = ({ title, data, hasLogs }) => {
  return (
    <div>
      <h2 className="mb-2 font-medium capitalize">{title}</h2>
      <div className="grid gap-4 xl:grid-cols-2">
        {map(data, (d) => (
          <ZoneDetailCard item={d} key={d?.node_id} hasLogs={hasLogs} />
        ))}
      </div>
    </div>
  )
}
