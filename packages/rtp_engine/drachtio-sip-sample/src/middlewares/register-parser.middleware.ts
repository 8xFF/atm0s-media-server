export function registerParserMiddleware(req: any, res: any, next: any) {
  if (req.method !== 'REGISTER') {
    return next()
  }

  const contact = req.getParsedHeader('contact')
  const to = req.getParsedHeader('to')
  const expireHeader = req.get('expires')

  if (!req.get('Contact') || !contact) {
    return res.send(400)
  }

  let expires = undefined
  if (contact[0].params && contact[0].params.expires) {
    expires = parseInt(contact[0].params.expires)
  } else if (
    typeof contact[0].params.expires === 'undefined' &&
    typeof expireHeader !== 'undefined'
  ) {
    expires = parseInt(expireHeader)
  } else {
    return res.send(400)
  }

  req.registration = {
    type: expires === 0 ? 'unregister' : 'register',
    expires,
    contact: contact,
    aor: to.uri,
  }

  next()
}
