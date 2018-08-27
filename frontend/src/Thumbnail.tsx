import * as React from 'react';
import Image from './model/Image';
import './Thumbnail.css';

export interface ThumbnailProps {
    image: Image;
    width: number;
    height: number;
    onClick: (image: Image) => void;
}

class Thumbnail extends React.Component<ThumbnailProps, any> {
  constructor(props: ThumbnailProps) {
    super(props);
    this.state = { };
  }

  public render() {
    const { image, width, height, onClick } = this.props;

    return (
      <div className="thumbnail">
        <img src={ `http://localhost:1080/image/${image.hash}/thumb` } width={ width } height={ height } onClick={ () => onClick(image) }/>
      </div>
    );
  }
}

export default Thumbnail;
