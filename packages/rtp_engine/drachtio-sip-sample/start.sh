#/bin/bash
export $(cat .env | xargs)
npm run build && npm run start
