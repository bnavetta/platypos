# Copied from https://github.com/Amanieu/mini-backtrace/blob/main/eh_frame.ld
__eh_frame_hdr_start = ADDR(.eh_frame_hdr);
__eh_frame_hdr_end = ADDR(.eh_frame_hdr) + SIZEOF(.eh_frame_hdr);
__eh_frame_start = ADDR(.eh_frame);
__eh_frame_end = ADDR(.eh_frame) + SIZEOF(.eh_frame);