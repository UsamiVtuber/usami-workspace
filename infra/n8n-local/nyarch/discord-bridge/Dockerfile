FROM node:22-alpine

RUN apk add --no-cache poppler-utils ffmpeg

WORKDIR /app

COPY package.json ./
RUN npm install --omit=dev --no-audit --no-fund

COPY index.mjs ./

USER node

CMD ["node", "index.mjs"]
