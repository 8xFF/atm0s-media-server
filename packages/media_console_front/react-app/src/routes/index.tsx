import {
  AuthSignIn,
  NetworkVisualization,
  Summary,
  ZonesDetail,
  ZonesEvents,
  ZonesList,
  ZonesPeers,
  ZonesRooms,
  ZonesSessions,
} from '@/containers'
import { Layout } from '@/layouts'
import { PrivateProvider } from '@/providers/private-provider'
import { createBrowserRouter } from 'react-router-dom'

export const routes = createBrowserRouter([
  {
    element: <PrivateProvider />,
    children: [
      {
        element: <Layout />,
        children: [
          { path: '/', element: <Summary /> },
          {
            path: '/zones',
            element: <ZonesList />,
          },
          {
            path: '/zones/:id',
            element: <ZonesDetail />,
          },
          {
            path: '/zones/rooms',
            element: <ZonesRooms />,
          },
          {
            path: '/zones/peers',
            element: <ZonesPeers />,
          },
          {
            path: '/zones/events',
            element: <ZonesEvents />,
          },
          {
            path: '/zones/sessions',
            element: <ZonesSessions />,
          },
          {
            path: '/network/visualization',
            element: <NetworkVisualization />,
          },
        ],
      },
    ],
  },
  {
    path: '/auth/sign-in',
    element: <AuthSignIn />,
  },
])
