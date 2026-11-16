# A plain static file server (server.py: http.server plus cache headers): PyBot itself runs in the visitor's browser (Pyodide, see worker.js).
# Docker rather than sdk: static because a static Space redirects large (LFS/Xet) files to a CDN without CORS
# headers, which blocks the worker's fetch of the bot bundle and the @font-face fonts.
FROM python:3.12-slim
RUN useradd -m -u 1000 user
USER user
WORKDIR /home/user/www
COPY --chown=user server.py index.html worker.js pybot_web.py tree.json pybot-*.zip ./
COPY --chown=user fonts ./fonts
EXPOSE 7860
CMD ["python", "server.py"]
