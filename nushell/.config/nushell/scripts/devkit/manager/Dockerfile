FROM oven/bun:1-slim
WORKDIR /app
COPY roles.ts server.ts ui.html ./
ENV PORT=8300
EXPOSE 8300
USER bun
CMD ["bun", "run", "server.ts"]
