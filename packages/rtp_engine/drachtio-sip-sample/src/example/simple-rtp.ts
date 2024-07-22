import { DRACHTIO_CONFIG, RTP_ENGINE_CONFIG } from 'config'
import * as Srf from 'drachtio-srf'
import { registerParserMiddleware } from 'middlewares'
const TmpSrf = require('drachtio-srf')
const Rtpengine = require('rtpengine-client').Client

export async function simpleRtp() {
  const parseUri = TmpSrf.parseUri
  const users = new Map()
  const srf = new Srf.default()
  const rtpengine = new Rtpengine()
  srf.connect(DRACHTIO_CONFIG)

  srf.on('connect', (err, hostPort) => {
    if (!err) {
      console.log(`connected to drachtio listening on ${hostPort}`)
    } else {
      console.log(`error connecting to drachtio: `, err)
    }
  })
  ;(srf as any).register(registerParserMiddleware, (req: any, res: any) => {
    const uri = parseUri(req.registration.aor)
    const headers: any = {}
    if (req.registration.type === 'unregister') {
      console.log(`unregistering ${uri.user}`)
      users.delete(uri.user)
    } else {
      const contact = req.registration.contact[0].uri
      users.set(uri.user, contact)
      console.log(`registering ${uri.user}`, contact)
      headers['Contact'] =
        `${req.get('Contact')};expires=${req.registration.expires || 300}`
    }

    res.send(200, {
      headers,
    })
  })

  srf.invite((req, res) => {
    const uri = parseUri(req.uri)
    // const from = req.get('From')
    const dest = users.get(uri.user)

    if (!dest) {
      return res.send(486, 'So sorry, busy right now', {})
    }

    const from = req.getParsedHeader('From') as any
    const details = {
      'call-id': req.get('Call-Id'),
      'from-tag': from.params.tag,
    }

    rtpengine
      .offer(RTP_ENGINE_CONFIG.port, RTP_ENGINE_CONFIG.host, {
        ...details,
        sdp: req.body,
      })
      .then((rtpRes: any) => {
        console.log(`got response from rtpengine: ${JSON.stringify(rtpRes)}`)
        if (rtpRes && rtpRes.result === 'ok') return rtpRes.sdp
        throw new Error('rtpengine failure')
      })
      .then((sdpB: any) => {
        console.log(`rtpengine offer returned sdp ${sdpB}`)
        return srf.createB2BUA(req, res, dest, {
          localSdpB: sdpB,
          localSdpA: getSdpA.bind(null, details),
        })
      })
      .then(({ uas, uac }: any) => {
        console.log('call connected with media proxy')
        return endCall(uas, uac, details)
      })
      .catch((err: any) => {
        console.error(`Error proxying call with media: ${err}: ${err.stack}`)
      })
  })

  function endCall(dlg1: any, dlg2: any, details: any) {
    let deleted = false
    ;[dlg1, dlg2].forEach((dlg) => {
      dlg.on('destroy', () => {
        ;(dlg === dlg1 ? dlg2 : dlg1).destroy()
        if (!deleted) {
          rtpengine.delete(
            RTP_ENGINE_CONFIG.port,
            RTP_ENGINE_CONFIG.host,
            details,
          )
          deleted = true
        }
      })
    })
  }

  function getSdpA(detail: any, remoteSdp: any, res: any) {
    return rtpengine
      .answer(RTP_ENGINE_CONFIG.port, RTP_ENGINE_CONFIG.host, {
        ...detail,
        sdp: remoteSdp,
        'to-tag': res.getParsedHeader('To').params.tag,
        ICE: 'remove',
      })
      .then((response: any) => {
        if (response.result !== 'ok')
          throw new Error(`Error calling answer: ${response['error-reason']}`)
        return response.sdp
      })
  }
}
