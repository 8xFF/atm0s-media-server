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
import { PrivateProvider } from '@/providers/private-provider'
import { createBrowserRouter } from 'react-router-dom'

export const routes = createBrowserRouter([
  {
    path: '/',
    element: (
      <PrivateProvider>
        <Summary />
      </PrivateProvider>
    ),
  },
  {
    path: '/auth/sign-in',
    element: <AuthSignIn />,
  },
  {
    path: '/zones',
    element: (
      <PrivateProvider>
        <ZonesList />
      </PrivateProvider>
    ),
  },
  {
    path: '/zones/:id',
    element: (
      <PrivateProvider>
        <ZonesDetail />
      </PrivateProvider>
    ),
  },
  {
    path: '/zones/rooms',
    element: (
      <PrivateProvider>
        <ZonesRooms />
      </PrivateProvider>
    ),
  },
  {
    path: '/zones/peers',
    element: (
      <PrivateProvider>
        <ZonesPeers />
      </PrivateProvider>
    ),
  },
  {
    path: '/zones/events',
    element: (
      <PrivateProvider>
        <ZonesEvents />
      </PrivateProvider>
    ),
  },
  {
    path: '/zones/sessions',
    element: (
      <PrivateProvider>
        <ZonesSessions />
      </PrivateProvider>
    ),
  },
  {
    path: '/network/visualization',
    element: (
      <PrivateProvider>
        <NetworkVisualization />
      </PrivateProvider>
    ),
  },
])
