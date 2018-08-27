import * as React from 'react';
import Image from './model/Image';
import './Lightbox.css';

export interface LightboxProps {
    image: Image;
    onNextImage: () => void;
    onPreviousImage: () => void;
    onClose: () => void;
}

interface LightboxState {
    width: number;
    height: number;
}

class Lightbox extends React.Component<LightboxProps, LightboxState> {
  private ref: React.RefObject<any>;

  constructor(props: LightboxProps) {
    super(props);
    this.state = {
      width: 0,
      height: 0,
    };
    this.ref = React.createRef();
  }

  public componentWillMount() {
    document.addEventListener("keydown", e => this.onKeyDown(e as KeyboardEvent), false);
  }

  public componentWillUnmount() {
    document.removeEventListener("keydown", e => this.onKeyDown(e as KeyboardEvent), false);
  }

  private onKeyDown(e: KeyboardEvent) {
    const { onNextImage, onPreviousImage } = this.props;
    if (e.key == "ArrowRight") {
        onNextImage();
    } else if (e.key == "ArrowLeft") {
        onPreviousImage();
    }
  }

  public componentDidMount() {
    const width = this.ref.current.offsetWidth;
    const height = this.ref.current.offsetHeight;

    this.setState({ width, height });
  }

  public render() {
    const { image } = this.props;
    const { width, height } = this.state;

    let imageWidth = image.width;
    let imageHeight = image.height;

    if (width != 0 && height != 0) {
      const maxWidth = 0.75*width;
      const maxHeight = 0.9*height;
      while (imageWidth > maxWidth || imageHeight > maxHeight) {
        imageWidth *= 0.99;
        imageHeight *= 0.99;
      }
    }

    return (
      <div className="lightbox" ref={ this.ref } onClick={ () => this.props.onClose() }>
        <div className="popup">
          <img src={ `http://localhost:1080/image/${image.hash}/preview` } width={ imageWidth } height={ imageHeight }/>
          <div className="info">
            lorem ipsum dolor sit amet
          </div>
        </div>
      </div>
    );
  }
}

export default Lightbox;
