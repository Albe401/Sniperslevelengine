FROM node:22-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev
COPY frontend/package*.json ./frontend/
RUN npm --prefix frontend ci
COPY . .
RUN npm run build
ENV HOST=0.0.0.0 PORT=8080
EXPOSE 8080
CMD ["node","--env-file-if-exists=.env","server/server.mjs"]
