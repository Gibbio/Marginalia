"""Tests for the sentence-aware chunking algorithm."""

from __future__ import annotations

from pathlib import Path

from marginalia_core.domain.document import build_document_outline


def test_short_paragraphs_are_merged_into_one_chunk() -> None:
    """Consecutive paragraphs that fit under the target become a single chunk."""

    text = "# Chapter\n\nShort line one.\n\nShort line two.\n\nShort line three."
    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=300)

    assert doc.chapter_count == 1
    section = doc.get_section(0)
    assert section.chunk_count == 1
    assert "Short line one." in section.get_chunk(0).text
    assert "Short line three." in section.get_chunk(0).text


def test_long_paragraph_is_split_at_sentence_boundaries() -> None:
    """A single paragraph exceeding 1.5x target is split into sentence chunks."""

    sentences = [
        "Prima frase di test che serve per riempire un po' di spazio.",
        "Seconda frase altrettanto lunga per raggiungere la soglia.",
        "Terza frase che porta il totale oltre il limite configurato.",
        "Quarta frase per garantire che il paragrafo venga spezzato.",
        "Quinta frase finale del paragrafo lungo.",
    ]
    long_paragraph = " ".join(sentences)
    text = f"# Chapter\n\n{long_paragraph}"

    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=120)

    section = doc.get_section(0)
    assert section.chunk_count > 1, (
        f"Expected multiple chunks but got {section.chunk_count}"
    )
    reconstructed = " ".join(chunk.text for chunk in section.chunks)
    for sentence in sentences:
        assert sentence in reconstructed


def test_very_short_target_produces_more_chunks() -> None:
    """A smaller target splits the same text into more chunks."""

    text = (
        "# Chapter\n\n"
        "First sentence here. Second sentence here. Third sentence here."
    )
    doc_wide = build_document_outline(Path("test.md"), text, chunk_target_chars=500)
    doc_narrow = build_document_outline(Path("test.md"), text, chunk_target_chars=30)

    assert doc_narrow.get_section(0).chunk_count > doc_wide.get_section(0).chunk_count


def test_default_target_merges_typical_prose_paragraphs() -> None:
    """With the default 300-char target, short prose paragraphs merge."""

    text = (
        "# Apertura\n\n"
        "Marco aprì la porta.\n\n"
        "La stanza era vuota.\n\n"
        "Si sedette e attese."
    )
    doc = build_document_outline(Path("test.md"), text)

    section = doc.get_section(0)
    assert section.chunk_count == 1
    assert "Marco aprì la porta." in section.get_chunk(0).text
    assert "Si sedette e attese." in section.get_chunk(0).text


def test_mixed_long_and_short_paragraphs() -> None:
    """Long paragraphs are split while adjacent short ones are merged."""

    short_1 = "Breve paragrafo iniziale."
    short_2 = "Altro paragrafo corto."
    long_para = (
        "Questo è un paragrafo molto lungo che contiene diverse frasi. "
        "Serve a verificare che il chunking lo spezzi correttamente. "
        "La terza frase aggiunge ulteriore materiale per superare la soglia. "
        "E la quarta frase lo porta decisamente oltre. "
        "Infine la quinta chiude il blocco."
    )
    short_3 = "Ultimo paragrafo breve."
    text = f"# Test\n\n{short_1}\n\n{short_2}\n\n{long_para}\n\n{short_3}"

    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=120)

    section = doc.get_section(0)
    all_text = " ".join(chunk.text for chunk in section.chunks)
    assert short_1 in all_text
    assert short_2 in all_text
    assert short_3 in all_text
    assert "Questo è un paragrafo molto lungo" in all_text


def test_char_offsets_are_monotonically_increasing() -> None:
    """Every chunk's char_start is >= the previous chunk's char_end."""

    text = (
        "# Chapter\n\n"
        "Prima frase. Seconda frase. Terza frase.\n\n"
        "Quarto paragrafo breve.\n\n"
        "Quinto paragrafo con un po' più di testo per variare."
    )
    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=50)

    section = doc.get_section(0)
    for i in range(1, section.chunk_count):
        prev = section.get_chunk(i - 1)
        curr = section.get_chunk(i)
        assert curr.char_start >= prev.char_end, (
            f"Chunk {i} start ({curr.char_start}) < chunk {i - 1} end ({prev.char_end})"
        )


def test_single_sentence_paragraph_stays_intact() -> None:
    """A paragraph with one sentence is never split, even if long."""

    long_sentence = "A" * 500
    text = f"# Chapter\n\n{long_sentence}"
    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=100)

    section = doc.get_section(0)
    assert section.chunk_count == 1
    assert section.get_chunk(0).text == long_sentence


def test_empty_section_produces_single_empty_chunk() -> None:
    """An empty section still produces one chunk for anchor stability."""

    text = "# Empty\n\n"
    doc = build_document_outline(Path("test.md"), text)

    section = doc.get_section(0)
    assert section.chunk_count == 1
    assert section.get_chunk(0).text == ""


def test_no_headings_creates_single_section() -> None:
    """Text without markdown headings becomes one section."""

    text = "Primo paragrafo.\n\nSecondo paragrafo.\n\nTerzo paragrafo."
    doc = build_document_outline(Path("test.md"), text)

    assert doc.chapter_count == 1
    assert doc.get_section(0).title == "Section 1"


def test_markdown_thematic_breaks_are_not_emitted_as_chunks() -> None:
    """Standalone markdown separators should not become readable chunks."""

    text = "# Title\n\nIntro paragraph.\n\n---\n\nSecond paragraph.\n\n***\n\nThird paragraph."
    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=120)

    section_text = " ".join(chunk.text for chunk in doc.get_section(0).chunks)
    assert "Intro paragraph." in section_text
    assert "Second paragraph." in section_text
    assert "Third paragraph." in section_text
    assert "---" not in section_text
    assert "***" not in section_text


def test_frontmatter_is_excluded_from_chunks() -> None:
    """YAML frontmatter should not appear in any chunk."""

    text = "---\ntitle: My Book\nauthor: Someone\n---\n\n# Chapter\n\nActual prose."
    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=300)

    all_text = " ".join(
        chunk.text for section in doc.sections for chunk in section.chunks
    )
    assert "Actual prose." in all_text
    assert "title:" not in all_text
    assert "author:" not in all_text


def test_code_fences_are_excluded_from_chunks() -> None:
    """Fenced code blocks should not leak into readable chunks."""

    text = (
        "# Chapter\n\n"
        "Some prose.\n\n"
        "```python\n# this is code not a heading\nprint('hello')\n```\n\n"
        "More prose."
    )
    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=300)

    all_text = " ".join(
        chunk.text for section in doc.sections for chunk in section.chunks
    )
    assert "Some prose." in all_text
    assert "More prose." in all_text
    assert "print(" not in all_text
    assert "# this is code" not in all_text


def test_inline_markup_is_stripped_from_chunks() -> None:
    """Bold, italic, links and inline code should become plain text."""

    text = (
        "# Chapter\n\n"
        "This has **bold** and *italic* words.\n\n"
        "A [link](http://example.com) and `inline code` here."
    )
    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=300)

    all_text = " ".join(chunk.text for chunk in doc.get_section(0).chunks)
    assert "bold" in all_text
    assert "**" not in all_text
    assert "*italic*" not in all_text
    assert "italic" in all_text
    assert "[link]" not in all_text
    assert "link" in all_text
    assert "http://example.com" not in all_text
    assert "inline code" in all_text
    assert "`" not in all_text


def test_heading_inside_code_fence_is_not_a_section() -> None:
    """A '#' inside a code fence must not create a section boundary."""

    text = (
        "# Real Chapter\n\n"
        "Intro.\n\n"
        "```\n# Fake heading\n```\n\n"
        "Conclusion."
    )
    doc = build_document_outline(Path("test.md"), text, chunk_target_chars=300)

    assert doc.chapter_count == 1
    all_text = " ".join(chunk.text for chunk in doc.get_section(0).chunks)
    assert "Intro." in all_text
    assert "Conclusion." in all_text
    assert "Fake heading" not in all_text


def test_voice_test_document_chunks_reasonably(tmp_path: Path) -> None:
    """The real voice-test-it.txt document produces reasonable chunk sizes."""

    voice_test = Path("examples/voice-test-it.txt")
    if not voice_test.exists():
        return
    raw_text = voice_test.read_text(encoding="utf-8")
    doc = build_document_outline(voice_test, raw_text, chunk_target_chars=300)

    for section in doc.sections:
        for chunk in section.chunks:
            if chunk.text:
                assert len(chunk.text) < 600, (
                    f"Chunk too long ({len(chunk.text)} chars) in "
                    f"'{section.title}': {chunk.text[:80]}..."
                )
