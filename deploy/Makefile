# /opt/kama-hub/Makefile — KM-DEPLOY-TDD-001 v0.5 §5. Tek satırlık hedefler, gizli bilgi yok; hepsi
# systemd üzerinden (elle `docker compose up -d` / `down` YOK). `deploy` kullanıcısının sudoers kapsamı
# §3.2'dedir.
#
#   make deploy TAG=<kısa-sha>   önceki etiketi .env.prev'e saklar, imajı çeker, servisi yeniden başlatır
#   make rollback                .env.prev'deki etiketle yeniden başlatır (< 1 dk, C5)
#   make status                  servis + konteyner + son 20 `hub stats` satırı

.PHONY: deploy rollback status

deploy:
	@test -n "$(TAG)" || { echo "usage: make deploy TAG=<short-sha>"; exit 2; }
	cp .env .env.prev
	echo HUB_TAG=$(TAG) > .env
	sudo docker compose -f compose.yml pull
	sudo systemctl restart kama-hub

rollback:
	cp .env.prev .env
	sudo systemctl restart kama-hub

status:
	systemctl status kama-hub --no-pager || true
	docker ps --filter name=hub || true
	journalctl -u kama-hub --no-pager | grep 'hub stats' | tail -n 20
