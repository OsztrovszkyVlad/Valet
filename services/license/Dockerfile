# Valet license service on Fly.io. Build context: services/license (run `fly deploy`
# from this directory). Only the service code goes into the image: no tests, no keys.
FROM node:22-alpine

ENV NODE_ENV=production
WORKDIR /app

COPY package.json server.mjs issue.mjs reset.mjs revoke.mjs lookup.mjs resend.mjs reissue.mjs undispute.mjs ./
COPY lib ./lib

# The volume is mounted at /data owned by root, which hides any ownership set in the
# image. The process starts as root only to hand /data to the image's `node` user
# (uid 1000, gid 1000), then switches to it (--run-as) before it opens the store or
# the port. Nothing runs as root after that.
RUN mkdir -p /data && chown node:node /data

EXPOSE 8080
CMD ["node", "server.mjs", "--host", "0.0.0.0", "--port", "8080", "--store", "/data/store.json", "--run-as", "1000:1000"]
