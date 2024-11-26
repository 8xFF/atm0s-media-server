import { ZoneDetailSection } from '@/components'
import { useDetailZoneQuery } from '@/hooks'
import { Layout } from '@/layouts'
import { Link, useParams } from 'react-router-dom'

export const ZonesDetail = () => {
  const params = useParams()
  const { data: dataDetailZone } = useDetailZoneQuery({
    payload: {
      zone_id: params?.id,
    },
    options: {
      enabled: !!params?.id,
    },
  })

  return (
    <Layout>
      <div className="grid gap-6">
        <Link
          to={`https://maps.google.com/?q=${dataDetailZone?.data?.lat},${dataDetailZone?.data?.lon}`}
          target="_blank"
          className="lg:text-md flex w-fit items-center gap-2 text-xs font-medium text-muted-foreground"
        >
          <div className="whitespace-nowrap">Lat: {dataDetailZone?.data?.lat}</div>|
          <div className="whitespace-nowrap">Lon: {dataDetailZone?.data?.lon}</div>
        </Link>
        <ZoneDetailSection title="connectors" data={dataDetailZone?.data?.connectors} hasLogs />
        <ZoneDetailSection title="consoles" data={dataDetailZone?.data?.consoles} />
        <ZoneDetailSection title="gateways" data={dataDetailZone?.data?.gateways} />
        <ZoneDetailSection title="medias" data={dataDetailZone?.data?.medias} />
      </div>
    </Layout>
  )
}
