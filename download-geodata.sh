mkdir -p maxminddb-data
cd maxminddb-data
rm -i GeoLite2-City.mmdb
wget https://github.com/P3TERX/GeoLite.mmdb/raw/download/GeoLite2-City.mmdb || { echo "Download GeoLite2-City database failed"; exit 1; }