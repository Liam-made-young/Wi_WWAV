/* Encodes a 16-bit WAV with a 44-byte header to Ogg Vorbis with libvorbis,
 * the reference encoder: `vorbis_encode in.wav out.ogg`. The tests build
 * this when libvorbis is installed, because the ffmpeg here has only its own
 * Vorbis encoder, which doesn't mark where a stream's sound ends. */
#include <stdio.h>
#include <vorbis/vorbisenc.h>

static void write_page(ogg_page *page, FILE *out) {
    fwrite(page->header, 1, page->header_len, out);
    fwrite(page->body, 1, page->body_len, out);
}

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    FILE *in = fopen(argv[1], "rb"), *out = fopen(argv[2], "wb");
    unsigned char h[44];
    if (!in || !out || fread(h, 1, 44, in) != 44) return 2;
    int channels = h[22];
    long rate = h[24] | h[25] << 8 | h[26] << 16 | (long)h[27] << 24;

    vorbis_info info;
    vorbis_comment comment;
    vorbis_dsp_state dsp;
    vorbis_block block;
    ogg_stream_state stream;
    ogg_packet packet, header[3];
    ogg_page page;
    vorbis_info_init(&info);
    if (vorbis_encode_init_vbr(&info, channels, rate, 0.5f)) return 1;
    vorbis_comment_init(&comment);
    vorbis_analysis_init(&dsp, &info);
    vorbis_block_init(&dsp, &block);
    /* A fixed serial number, so the same sound gives the same file. */
    ogg_stream_init(&stream, 1234);
    vorbis_analysis_headerout(&dsp, &comment, &header[0], &header[1], &header[2]);
    for (int i = 0; i < 3; i++) ogg_stream_packetin(&stream, &header[i]);
    while (ogg_stream_flush(&stream, &page)) write_page(&page, out);

    short frames[1024 * 2];
    for (int ended = 0; !ended;) {
        size_t n = fread(frames, 2 * channels, 1024, in);
        float **buffer = vorbis_analysis_buffer(&dsp, 1024);
        for (size_t i = 0; i < n; i++)
            for (int c = 0; c < channels; c++)
                buffer[c][i] = frames[i * channels + c] / 32768.f;
        /* Writing no frames ends the stream. */
        vorbis_analysis_wrote(&dsp, n);
        while (vorbis_analysis_blockout(&dsp, &block) == 1) {
            vorbis_analysis(&block, NULL);
            vorbis_bitrate_addblock(&block);
            while (vorbis_bitrate_flushpacket(&dsp, &packet)) {
                ogg_stream_packetin(&stream, &packet);
                while (!ended && ogg_stream_pageout(&stream, &page)) {
                    write_page(&page, out);
                    ended = ogg_page_eos(&page);
                }
            }
        }
    }
    return fclose(out) != 0;
}
