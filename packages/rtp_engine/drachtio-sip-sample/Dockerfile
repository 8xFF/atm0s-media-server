FROM node:18.18-alpine as builder
WORKDIR /usr/app
COPY package.json ./
COPY package-lock.json ./
RUN npm install --frozen-lockfile
COPY . .
RUN npm run build

FROM node:18.18-alpine
WORKDIR /usr/app
ENV NODE_ENV production
ENV NODE_PATH dist/
COPY package.json ./
COPY package-lock.json ./
RUN npm install --frozen-lockfile --production
COPY --from=builder /usr/app/dist ./dist
CMD ["node", "dist/index.js"]
