# EconGraph Professional Demo Resources

EconGraph brings economic series search, interactive charts, and shared research notes into one responsive interface. A presentation can follow an analyst from finding a series to examining its history and discussing an observation with colleagues. This page connects that presentation theme with the demo recorder, orchestration script, and checked-in media.

## Presentation audience and value

Financial analysts can use EconGraph to explore indicators for investment research and reporting. Economic research organizations, government agencies, and academic institutions can use the same interface to examine trends, monitor policy-relevant series, and teach economic analysis. Public and private chart annotations let signed-in users preserve context alongside the data; comment threads on public annotations support discussion with other readers.

For current product capabilities, see the [product overview](../README.md) and [frontend feature guide](../docs/technical/FRONTEND_SUMMARY.md#frontend-features). The guide covers series charts and revision information, search filters, and responsive navigation. These provide concrete presentation topics without relying on the recorder's older authentication and analysis screens.

## Recorder and presentation sequence

The [Playwright recorder](create-professional-demo-video.js) targets `http://localhost:3000`. It configures a 1920×1080 desktop viewport and recording size, switches the viewport to 375×812 for a mobile layout segment, and then returns to desktop. The recording canvas remains 1920×1080 during the viewport changes.

Its sequence introduces the interface, attempts sign-in and analysis interactions, describes chart analytics and collaboration, switches to the mobile viewport, and enters a search query. Element highlights and text captions focus attention on the topics being discussed. The 19 caption entries run sequentially, with waits based on each entry's duration; their `timestamp` fields do not schedule playback. The recorder does not generate speech or mix an audio track.

The scripted interactions differ from the current application in several important ways:

- **Sign-in:** The recorder expects an in-app dialog with Google and Facebook buttons and an email sign-up form. It fills the form and closes the dialog to simulate login, without submitting registration or completing authentication. The application redirects sign-in through Keycloak using OpenID Connect with PKCE; see [application setup](../README.md) for identity-provider configuration.
- **Analysis:** The recorder navigates to `/analysis`, which is absent from the current router. Current series exploration and charts are available through `/explore` and `/series/:id`, as described in the [frontend feature guide](../docs/technical/FRONTEND_SUMMARY.md#frontend-features).
- **Collaboration:** The recorder displays captions about annotations and comment threads and conditionally opens a collaboration panel. It does not create shared annotations or exchange comments between users. Series annotations and comments are current product capabilities, described in the [product overview](../README.md).

Some highlight selectors also use Playwright syntax such as `:has-text` inside native `document.querySelector` calls, which cannot interpret that syntax. The script therefore serves as a reference for its presentation sequence rather than a working demonstration of every captioned feature.

## Checked-in media

The [demo asset](professional-econgraph-demo-v2.mp4) contains a 1920×1080 VP8 video stream in a WebM container, with a duration of 0.040 seconds and no audio stream. It does not contain the full walkthrough described by the recorder.

The filename comes from the recorder's finalization step: it selects a `.webm` file from `./videos/`, renames it to `professional-econgraph-demo-v2.mp4`, and removes the video directory. Renaming does not transcode the container to MP4. After a successful recording, the recorder also writes `PROFESSIONAL_DEMO_README.md` in its working directory.

## Orchestration and repository effects

The [orchestration script](professional-demo-orchestrator.sh) sequences prerequisite checks, PostgreSQL startup, local backend and frontend startup, Playwright dependency installation, recording, Git publication, and a README edit. Its path assumptions require both the recorder and `docker-compose.yml` in the working directory, with `backend/` and `frontend/` beneath it. The repository places the recorder in `demo-tools/` and those application resources at the repository root, so neither directory satisfies those assumptions. Use the [application setup guide](../README.md) for running EconGraph.

The script's publication step stages the media, generated demo documentation, recorder, and orchestrator when the working tree has changes, commits them, and runs `git push origin main`. It then prepends a demo section to a local `README.md` if that section is absent. Because the README edit follows the push, that edit is not included in the preceding publication step. The script creates no GitHub release and uploads no release asset.

On exit, its cleanup trap runs `docker-compose down --remove-orphans` when the compose file is present. It also uses `pkill -f` for command lines matching `npm start`, `cargo run`, and `node create-professional-demo-video.js`, which can stop matching processes beyond those started by the orchestrator.

[Return to the demo tools index](README.md).
