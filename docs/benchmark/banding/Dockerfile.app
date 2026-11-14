# Aplikasi yang sama di atas tiap server: DASAR menentukan servernya.
ARG DASAR
FROM ${DASAR}
ARG PEMILIK=www-data
COPY --chown=${PEMILIK}:${PEMILIK} . /app
ARG SIAPKAN=true
RUN sh -c "$SIAPKAN"
