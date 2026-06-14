KAPITOLY := $(sort $(wildcard kapitoly/*.md))

epub:
	pandoc metadata.yaml $(KAPITOLY) \
		--output build/kniha.epub \
		--toc \
		--toc-depth=2 \
		--highlight-style=breezedark \
		--css=epub.css \
		--epub-embed-font=/usr/share/fonts/truetype/noto/NotoSerif-Regular.ttf \
		--epub-embed-font=/usr/share/fonts/truetype/noto/NotoSerif-Bold.ttf \
		--epub-embed-font=/usr/share/fonts/truetype/noto/NotoSerif-Italic.ttf \
		--epub-embed-font=/usr/share/fonts/truetype/noto/NotoSerif-BoldItalic.ttf \
		--epub-cover-image=cover.png

pdf:
	pandoc metadata.yaml $(KAPITOLY) \
		--output build/kniha.pdf \
		--toc \
		--highlight-style=kate \
		--pdf-engine=xelatex

check:
	cd priklady && cargo check --workspace

test:
	cd priklady && cargo test --workspace

all: epub pdf

.PHONY: epub pdf check test all
