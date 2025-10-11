MDS := $(wildcard *.md)

all: $(MDS:.md=.pdf)

%.pdf: %.md *.tex
	pandoc $< -o $@ --pdf-engine=lualatex -V documentclass=ltjsarticle -H header.tex
