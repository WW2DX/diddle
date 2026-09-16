// Explicit microphone-permission request for macOS. Capturing through
// CoreAudio from a background thread triggers the TCC prompt implicitly,
// which in a Tauri app can leave a dialog that never dismisses; asking up
// front through AVFoundation, and only touching audio devices once the
// answer is in, avoids that.
#import <AVFoundation/AVFoundation.h>

typedef void (*diddle_mic_cb)(int granted, void *ctx);

// 0 = not determined, 1 = restricted, 2 = denied, 3 = authorized.
int diddle_mic_status(void) {
    return (int)[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
}

void diddle_mic_request(diddle_mic_cb cb, void *ctx) {
    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio
                             completionHandler:^(BOOL granted) {
                                 cb(granted ? 1 : 0, ctx);
                             }];
}
