import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useZonesQuery } from '@/hooks'
import { useTheme } from '@/providers'
import { forEach, sumBy } from 'lodash'
import { Hash } from 'lucide-react'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import { useEffect, useRef, useState } from 'react'

const MAPBOX_TOKEN = 'pk.eyJ1IjoiY2FvaGF2YW4iLCJhIjoiY2x5anNkcDBzMGw2bTJqcGF4OTNjbTk1dCJ9.quX_1lfj-fPC8hNzpwUWiA'

type Feature = {
  type: string
  properties: { id: number }
  geometry: { type: string; coordinates: [number, number, number] }
}
export const Summary = () => {
  const { theme } = useTheme()
  const [detectTheme, setDetectTheme] = useState<string>('light')
  const { data: zones } = useZonesQuery({})

  const totalZones = zones?.data?.length
  const totalGateways = sumBy(zones?.data, 'gateways')
  const totalMedias = sumBy(zones?.data, 'medias')
  const totalConnectors = sumBy(zones?.data, 'connectors')
  const totalConsoles = sumBy(zones?.data, 'consoles')

  useEffect(() => {
    if (theme === 'system') {
      const classTheme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
      setDetectTheme(classTheme)
    } else {
      setDetectTheme(theme || 'light')
    }
  }, [theme])

  const mapContainerRef = useRef<any>(null)
  const mapRef = useRef<any>(null)
  useEffect(() => {
    if (!zones) return
    mapboxgl.accessToken = MAPBOX_TOKEN
    mapRef.current = new mapboxgl.Map({
      container: mapContainerRef.current,
      style: `mapbox://styles/mapbox/${detectTheme}-v10`,
      center: [105.8342, 21.0278],
      zoom: 6,
    })

    let arrayFeature: Feature[] = []
    forEach(zones?.data, (item) => {
      const data: Feature = {
        type: 'Feature',
        properties: { id: item.zone_id, ...zones },
        geometry: { type: 'Point', coordinates: [item.lon, item.lat, 0.0] },
      }
      arrayFeature = [...(arrayFeature || []), data]
    })
    const temp = {
      type: 'FeatureCollection',
      crs: { type: 'name' },
      features: arrayFeature,
    }
    mapRef.current.on('load', () => {
      mapRef.current.addSource('zones', {
        type: 'geojson',
        data: temp,
        cluster: true,
        clusterMaxZoom: 14,
        clusterRadius: 50,
      })
      // cricle big
      mapRef.current.addLayer({
        id: 'clusters',
        type: 'circle',
        source: 'zones',
        filter: ['has', 'point_count'],
        paint: {
          'circle-color': ['step', ['get', 'point_count'], '#51bbd6', 100, '#f1f075', 750, '#f28cb1'],
          'circle-radius': ['step', ['get', 'point_count'], 20, 100, 30, 750, 40],
        },
      })
      // number
      mapRef.current.addLayer({
        id: 'cluster-count',
        type: 'symbol',
        source: 'zones',
        filter: ['has', 'point_count'],
        layout: {
          'text-field': ['get', 'point_count_abbreviated'],
          'text-font': ['DIN Offc Pro Medium', 'Arial Unicode MS Bold'],
          'text-size': 12,
        },
      })
      // cricle small
      mapRef.current.addLayer({
        id: 'unclustered-point',
        type: 'circle',
        source: 'zones',
        filter: ['!', ['has', 'point_count']],
        paint: {
          'circle-color': '#11b4da',
          'circle-radius': 4,
          'circle-stroke-width': 1,
          'circle-stroke-color': '#fff',
        },
      })
      // inspect a cluster on click
      mapRef.current.on('click', 'clusters', (e: any) => {
        const features = mapRef.current.queryRenderedFeatures(e.point, {
          layers: ['clusters'],
        })
        const clusterId = features[0].properties.cluster_id
        mapRef.current.getSource('zones').getClusterExpansionZoom(clusterId, (err: any, zoom: any) => {
          if (err) return

          mapRef.current.easeTo({
            center: features[0].geometry.coordinates,
            zoom: zoom,
          })
        })
      })

      // When a click event occurs on a feature in
      // the unclustered-point layer, open a popup at
      // the location of the feature, with
      // description HTML from its properties.
      mapRef.current.on('click', 'unclustered-point', (e: any) => {
        const coordinates = e.features[0].geometry.coordinates.slice()
        const consoles = sumBy(e.features[0].properties.zones?.data, 'consoles')
        const gateways = sumBy(zones?.data, 'gateways')
        const medias = sumBy(zones?.data, 'medias')
        const connectors = sumBy(zones?.data, 'connectors')
        // Ensure that if the map is zoomed out such that
        // multiple copies of the feature are visible, the
        // popup appears over the copy being pointed to.
        while (Math.abs(e.lngLat.lng - coordinates[0]) > 180) {
          coordinates[0] += e.lngLat.lng > coordinates[0] ? 360 : -360
        }

        new mapboxgl.Popup()
          .setLngLat(coordinates)
          .setHTML(
            `<span style="color:#000;">Consoles: ${consoles}</span><br><span style="color:#000;">Gateways: ${gateways}</span><br><span style="color:#000;">Medias: ${medias}</span><br><span style="color:#000;">Connectors: ${connectors}</span>`
          )
          .addTo(mapRef.current)
      })

      mapRef.current.on('mouseenter', 'clusters', () => {
        mapRef.current.getCanvas().style.cursor = 'pointer'
      })
      mapRef.current.on('mouseleave', 'clusters', () => {
        mapRef.current.getCanvas().style.cursor = ''
      })
    })

    return () => mapRef.current.remove()
  }, [detectTheme, zones])

  return (
    <>
      <div className="grid gap-6">
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5">
          <Card className="shadow-xs">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Total Zones</CardTitle>
              <Hash className="text-muted-foreground h-4 w-4" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{totalZones}</div>
            </CardContent>
          </Card>
          <Card className="shadow-xs">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Total Gateways</CardTitle>
              <Hash className="text-muted-foreground h-4 w-4" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{totalGateways}</div>
            </CardContent>
          </Card>
          <Card className="shadow-xs">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Total Medias</CardTitle>
              <Hash className="text-muted-foreground h-4 w-4" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{totalMedias}</div>
            </CardContent>
          </Card>
          <Card className="shadow-xs">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Total Connectors</CardTitle>
              <Hash className="text-muted-foreground h-4 w-4" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{totalConnectors}</div>
            </CardContent>
          </Card>
          <Card className="shadow-xs">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">Total Consoles</CardTitle>
              <Hash className="text-muted-foreground h-4 w-4" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{totalConsoles}</div>
            </CardContent>
          </Card>
        </div>
        <div className="relative h-[70vh] overflow-hidden rounded-lg">
          <div id="map" ref={mapContainerRef} className="absolute top-0 left-0 h-full w-full" />
        </div>
      </div>
    </>
  )
}
