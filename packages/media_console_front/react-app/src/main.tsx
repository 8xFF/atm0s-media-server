import { AppProvider } from '@/providers'
import { createRoot } from 'react-dom/client'
import './index.css'

createRoot(document.getElementById('root')!).render(<AppProvider />)
