import path from 'path'
import * as dotenv from 'dotenv'

const envPath = path.join(process.cwd(), '.env')
dotenv.config({
  path: envPath,
  override: true,
})

export const ENV = process.env.ENV || 'develop'
export const RUN_MODE = process.env.RUN_MODE || 'rtp'

export const DRACHTIO_CONFIG = {
  host: process.env.DRACHTIO_HOST || '127.0.0.1',
  port: parseInt(process.env.DRACHTIO_PORT || '9022'),
  secret: process.env.DRACHTIO_SECRET || '',
}

export const MRF_CONFIG = {
  address: process.env.MRF_ADDRESS || '127.0.0.1',
  port: parseInt(process.env.MRF_PORT || '8021'),
  secret: process.env.MRF_SECRET || '',
}

export const RTP_ENGINE_CONFIG = {
  host: process.env.RTP_ENGINE_HOST || '127.0.0.1',
  port: parseInt(process.env.RTP_ENGINE_PORT || '22222'),
}
