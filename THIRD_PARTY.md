# Third-party components

The original application source is MIT-licensed. The repository also includes:

- nlohmann/json (`json.hpp`), MIT; notice in `third-party-licenses/nlohmann-json-MIT.txt`.
- Notices for Zig/LLVM C++ runtime and MinGW components in `third-party-licenses`.

Optional video helpers are downloaded by `provision-video.py`; their executables are not included in this source repository:

- yt-dlp: https://github.com/yt-dlp/yt-dlp ; release binaries can include additional third-party code/notices.
- Deno: https://github.com/denoland/deno .
- FFmpeg: https://git.ffmpeg.org/ffmpeg.git ; the script selects the GPL shared build distributed by https://github.com/yt-dlp/FFmpeg-Builds .

The provisioner records selected release URLs and checksums in `video-runtime/versions.json` and puts collected notices in `video-runtime/licenses`. Include applicable notices when distributing generated binaries. Preserve exact dependency versions and their corresponding source/build information for releases. This repository's app source alone is not the corresponding source for separately distributed GPL helper binaries; obtain and provide the corresponding upstream source and build information as required by the chosen distribution's license.

Windows WinHTTP, curl and other Windows APIs are provided by the operating system and are not copied into this repository.