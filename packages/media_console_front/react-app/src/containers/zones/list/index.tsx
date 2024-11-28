import { useZonesQuery } from '@/hooks'
import { Layout } from '@/layouts'
import { map } from 'lodash'
import { ZoneItem } from './components'

export const ZonesList = () => {
  const { data: dataZones } = useZonesQuery({})

  return (
    <Layout>
      <div className="grid gap-4 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
        {map(dataZones?.data, (zone) => (
          <ZoneItem zone={zone} key={zone?.zone_id} />
        ))}
      </div>
    </Layout>
  )
}
