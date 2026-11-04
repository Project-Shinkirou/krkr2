#include "platform/CCGLView.h"
#import <OpenGLES/EAGL.h>
#import <OpenGLES/ES2/gl.h>
#import <OpenGLES/ES2/glext.h>
#include <memory>
#include <stdexcept>

namespace shinki::krkr {
namespace {
class EmbeddedIOSView final : public cocos2d::GLView {
    __strong EAGLContext *context_ = nil;
    GLuint framebuffer_ = 0;
    GLuint color_ = 0;
    GLuint depth_stencil_ = 0;

public:
    static EmbeddedIOSView *create() {
        auto view = std::unique_ptr<EmbeddedIOSView>(new EmbeddedIOSView());
        view->context_ = [[EAGLContext alloc] initWithAPI:kEAGLRenderingAPIOpenGLES2];
        if(!view->context_ || ![EAGLContext setCurrentContext:view->context_])
            throw std::runtime_error("SHINKI_EMU_KRKR_NATIVE_GL_CONTEXT");

        const auto attrs = getGLContextAttrs();
        const auto storage = [&](GLenum format) {
            if(attrs.multisamplingCount > 1)
                glRenderbufferStorageMultisampleAPPLE(GL_RENDERBUFFER, attrs.multisamplingCount,
                                                      format, 960, 640);
            else
                glRenderbufferStorage(GL_RENDERBUFFER, format, 960, 640);
        };
        glGenFramebuffers(1, &view->framebuffer_);
        glBindFramebuffer(GL_FRAMEBUFFER, view->framebuffer_);
        glGenRenderbuffers(1, &view->color_);
        glBindRenderbuffer(GL_RENDERBUFFER, view->color_);
        storage(GL_RGBA8_OES);
        glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0,
                                  GL_RENDERBUFFER, view->color_);
        if(attrs.depthBits || attrs.stencilBits) {
            glGenRenderbuffers(1, &view->depth_stencil_);
            glBindRenderbuffer(GL_RENDERBUFFER, view->depth_stencil_);
            storage(attrs.stencilBits ? GL_DEPTH24_STENCIL8_OES
                                     : attrs.depthBits > 16 ? GL_DEPTH_COMPONENT24_OES
                                                            : GL_DEPTH_COMPONENT16);
            if(attrs.depthBits)
                glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT,
                                          GL_RENDERBUFFER, view->depth_stencil_);
            if(attrs.stencilBits)
                glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_STENCIL_ATTACHMENT,
                                          GL_RENDERBUFFER, view->depth_stencil_);
        }
        if(glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE)
            throw std::runtime_error("SHINKI_EMU_KRKR_NATIVE_GL_FRAMEBUFFER");
        view->setViewName("Shinki KrKr context");
        view->setFrameSize(960, 640);
        view->autorelease();
        return view.release();
    }

    ~EmbeddedIOSView() override {
        if(!context_) return;
        EAGLContext *previous = [EAGLContext currentContext];
        if([EAGLContext setCurrentContext:context_]) {
            if(depth_stencil_) glDeleteRenderbuffers(1, &depth_stencil_);
            if(color_) glDeleteRenderbuffers(1, &color_);
            if(framebuffer_) glDeleteFramebuffers(1, &framebuffer_);
        }
        [EAGLContext setCurrentContext:previous == context_ ? nil : previous];
    }
    void end() override { release(); }
    bool isOpenGLReady() override {
        return context_ && [EAGLContext currentContext] == context_;
    }
    void swapBuffers() override {
        // The native game texture is read by the existing CPU frame lease;
        // only Manager presents. No UIKit view or second event owner is created.
        glFlush();
    }
    void setIMEKeyboardState(bool open) override {
        if(open) throw std::runtime_error("SHINKI_EMU_KRKR_NATIVE_IME_UNSUPPORTED");
    }
};
}

cocos2d::GLView *create_embedded_ios_view() {
    return EmbeddedIOSView::create();
}
}
