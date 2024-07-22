import { RUN_MODE } from 'config'
import { initFreeswitch } from './freeswitch'
import { simpleRtp } from './simple-rtp'

enum MODE {
  FREESWITCH = 'freeswitch',
  RTP = 'rtp',
}

export function executeFunc() {
  if ((RUN_MODE as MODE) === MODE.FREESWITCH) {
    return initFreeswitch
  } else {
    return simpleRtp
  }
}
